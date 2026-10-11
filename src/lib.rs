//! WDE MCP server — exposes the Worst-Day-Ever skill over MCP as BOTH:
//!   * a tool  `worst_day_ever_assess`  (runs the assessment workflow)
//!   * a skill `skill://worst-day-ever/SKILL.md` (SEP-2640 skills extension)
//!
//! Built on the rmcp SDK (SEP-2640 `feat/sep-2640-skills` branch).
//!
//! Two transports share this implementation:
//!   * `wde-mcp-stdio` — stdio (for MCP Inspector CLI / desktop clients)
//!   * `wde-mcp-http`  — streamable HTTP on 127.0.0.1:3001/mcp (for Inspector web UI)

use std::{collections::HashMap, fs, path::Path, sync::Arc};

use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::wrapper::Parameters,
    model::skills::{
        self, SkillEntry, SkillResource, SkillResources, SkillsGetRequestParams, SkillsGetResult,
        SkillsListResult,
    },
    model::{
        CallToolResult, ContentBlock, Implementation, InitializeResult, PaginatedRequestParams,
        ProtocolVersion, ReadResourceRequestParams, ReadResourceResponse, ReadResourceResult,
        Resource, ResourceContents, ServerCapabilities,
    },
    schemars,
    service::{RequestContext, RoleServer},
    tool, tool_handler, tool_router,
};

/// A skill directory entry: URI suffix -> raw bytes of the file.
type SkillFiles = HashMap<String, Vec<u8>>;

/// The catalog a server is built from: skills by URI, and their files by root URI.
type SkillCatalog = (HashMap<String, SkillEntry>, HashMap<String, SkillFiles>);

// ===========================================================================
// Server
// ===========================================================================

#[derive(Debug, Clone)]
pub struct WdeServer {
    /// skill URI -> parsed skill entry
    skills: Arc<HashMap<String, SkillEntry>>,
    /// skill URI -> (relative path -> file bytes) for resources/directory/read
    skill_files: Arc<HashMap<String, SkillFiles>>,
}

impl WdeServer {
    /// Build the server from a root directory that contains `worst-day-ever/`.
    pub fn from_root(root: impl AsRef<Path>) -> Result<Self, std::io::Error> {
        let root = root.as_ref();
        let (skills, skill_files) = Self::discover_skills(root)?;
        Ok(Self {
            skills: Arc::new(skills),
            skill_files: Arc::new(skill_files),
        })
    }

    /// Discover every `SKILL.md` under `root` and index its sibling files.
    fn discover_skills(root: &Path) -> Result<SkillCatalog, std::io::Error> {
        let mut skills = HashMap::new();
        let mut files: HashMap<String, SkillFiles> = HashMap::new();
        let canonical_root = root.canonicalize()?;
        Self::walk(&canonical_root, &canonical_root, &mut skills, &mut files)?;
        Ok((skills, files))
    }

    fn walk(
        root: &Path,
        dir: &Path,
        skills: &mut HashMap<String, SkillEntry>,
        files: &mut HashMap<String, SkillFiles>,
    ) -> Result<(), std::io::Error> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                Self::walk(root, &path, skills, files)?;
                continue;
            }
            if path.file_name().and_then(|s| s.to_str()) != Some("SKILL.md") {
                continue;
            }
            if let Some((entry_skill, dir_files)) = Self::parse_skill(root, &path) {
                // Key the file map by the skill ROOT uri (`skill://<rel>`),
                // which is what `resources/directory/read` requests carry.
                // The `skills` map stays keyed by the full `.../SKILL.md` uri.
                let root_uri = entry_skill
                    .uri
                    .strip_suffix("/SKILL.md")
                    .unwrap_or(&entry_skill.uri)
                    .to_string();
                files.insert(root_uri, dir_files);
                skills.insert(entry_skill.uri.clone(), entry_skill);
            }
        }
        Ok(())
    }

    /// Parse one `SKILL.md`, collect its sibling files, and build a `SkillEntry`.
    fn parse_skill(root: &Path, skill_md: &Path) -> Option<(SkillEntry, SkillFiles)> {
        let content = fs::read_to_string(skill_md).ok()?;
        let frontmatter = Self::frontmatter_to_json(&content);

        let parent = skill_md.parent()?;
        let rel_parent = parent.strip_prefix(root).ok()?;
        let uri = format!("skill://{}/SKILL.md", rel_parent.to_string_lossy());

        // Collect sibling files (recursively) as resources.
        let mut resources = Vec::new();
        let mut dir_files = HashMap::new();
        Self::collect_files(root, parent, &mut resources, &mut dir_files);

        let resources = if resources.is_empty() {
            None
        } else {
            Some(SkillResources::FileList(resources))
        };

        let mut entry = SkillEntry::new(uri.clone(), frontmatter);
        entry.resources = resources;
        Some((entry, dir_files))
    }

    fn collect_files(
        root: &Path,
        dir: &Path,
        resources: &mut Vec<SkillResource>,
        out: &mut SkillFiles,
    ) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                Self::collect_files(root, &path, resources, out);
                continue;
            }
            // Include SKILL.md itself: the spec requires `resources` to be
            // complete, and it is the only way an agent can read the skill's
            // body (`skills/get` returns frontmatter only).
            let Ok(rel) = path.strip_prefix(root) else {
                continue;
            };
            let rel_str = rel.to_string_lossy().to_string();
            let uri = format!("skill://{}", rel_str);
            let Ok(bytes) = fs::read(&path) else {
                continue;
            };
            let size = bytes.len() as u64;
            resources.push(SkillResource::new(
                uri,
                format!("sha256:{}", Self::hex_len(size)),
                size,
            ));
            out.insert(rel_str, bytes);
        }
        resources.sort_by(|a, b| a.uri.cmp(&b.uri));
    }

    /// Cheap, non-cryptographic stand-in digest marker. The spec treats the
    /// digest as an opaque cache key; a real deployment should use SHA-256.
    fn hex_len(size: u64) -> String {
        format!("{:064x}", size)
    }

    /// Parse `---`-delimited YAML frontmatter into a JSON object.
    /// Handles `key: value` scalars and block scalars (`key: |`) — the WDE
    /// SKILL.md uses a block scalar for its `description`.
    fn frontmatter_to_json(content: &str) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        let Some(rest) = content.strip_prefix("---\n") else {
            return serde_json::Value::Object(map);
        };
        let lines = rest.lines();
        let mut block_key: Option<String> = None;
        let mut block_lines: Vec<String> = Vec::new();

        for line in lines {
            if line.trim() == "---" {
                break;
            }
            // Inside a block scalar: consume indented lines.
            if let Some(key) = block_key.clone() {
                if line.starts_with(' ') || line.starts_with('\t') {
                    block_lines.push(line.trim_start().to_string());
                    continue;
                }
                map.insert(
                    key,
                    serde_json::Value::String(block_lines.join("\n").trim().to_string()),
                );
                block_key = None;
                block_lines.clear();
            }
            let Some((k, v)) = line.split_once(':') else {
                continue;
            };
            let k = k.trim();
            if k.is_empty() {
                continue;
            }
            let v = v.trim();
            if v == "|" || v == ">" || v == "|-" || v == ">-" {
                block_key = Some(k.to_string());
                continue;
            }
            let v = v.trim_matches(|c| c == '"' || c == '\'');
            map.insert(k.to_string(), serde_json::Value::String(v.to_string()));
        }
        if let Some(key) = block_key {
            map.insert(
                key,
                serde_json::Value::String(block_lines.join("\n").trim().to_string()),
            );
        }
        serde_json::Value::Object(map)
    }

    /// The skill URI for the bundled WDE skill, if present.
    pub fn wde_uri(&self) -> Option<String> {
        self.skills
            .keys()
            .find(|u| u.contains("worst-day-ever"))
            .cloned()
    }

    /// All discovered skills, sorted by URI.
    pub fn list_skills(&self) -> Vec<&SkillEntry> {
        let mut skills: Vec<&SkillEntry> = self.skills.values().collect();
        skills.sort_by(|a, b| a.uri.cmp(&b.uri));
        skills
    }

    /// Look up one skill by URI, accepting either the canonical
    /// `skill://<path>/SKILL.md` form or the bare `skill://<path>` root form.
    pub fn get_skill(&self, uri: &str) -> Option<&SkillEntry> {
        if let Some(skill) = self.skills.get(uri) {
            return Some(skill);
        }
        let normalized = format!("{}/SKILL.md", uri.trim_end_matches('/'));
        self.skills.get(&normalized)
    }

    /// Read the raw bytes of a skill file by its `skill://` URI.
    ///
    /// This is the only path to a skill's *body*: `skills/get` returns
    /// frontmatter only, so an agent that needs the instructions reads them
    /// here. Returns `None` when the URI is not an advertised resource.
    pub fn read_skill_file(&self, uri: &str) -> Option<&Vec<u8>> {
        let rel = uri.strip_prefix("skill://")?;
        self.skill_files.values().find_map(|files| files.get(rel))
    }
}

// ===========================================================================
// Tool: worst_day_ever_assess
// ===========================================================================

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct AssessParams {
    /// Path to the project to assess.
    #[schemars(description = "Absolute path to the project directory to stress-test")]
    pub project_path: String,
    /// Optional label used in the report title.
    #[schemars(description = "Human-readable project name (defaults to the directory name)")]
    pub project_name: Option<String>,
}

#[tool_router]
impl WdeServer {
    /// Run the Worst-Day-Ever adversarial assessment workflow against a project.
    #[tool(
        name = "worst_day_ever_assess",
        description = "Run the Worst-Day-Ever adversarial stress-test workflow against a project. \
                       Returns the WDE skill's dimension checklist, safeguards, and a per-project \
                       reconnaissance summary (language mix, entry points, test hooks)."
    )]
    fn worst_day_ever_assess(
        &self,
        Parameters(AssessParams {
            project_path,
            project_name,
        }): Parameters<AssessParams>,
    ) -> Result<CallToolResult, McpError> {
        let path = Path::new(&project_path);
        if !path.exists() {
            return Err(McpError::invalid_params(
                format!("project_path does not exist: {}", project_path),
                None,
            ));
        }
        if !path.is_dir() {
            return Err(McpError::invalid_params(
                format!("project_path is not a directory: {}", project_path),
                None,
            ));
        }

        let name = project_name.unwrap_or_else(|| {
            path.file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("project")
                .to_string()
        });

        let recon = self.recon(path);
        let skill_uri = self
            .wde_uri()
            .unwrap_or_else(|| "skill://worst-day-ever/SKILL.md".into());

        let body = format!(
            "# Worst-Day-Ever assessment: {name}\n\n\
             **Project path:** `{p}`\n\
             **Skill:** `{skill_uri}`\n\n\
             ## Reconnaissance\n\n\
             - Source files: {src}\n\
             - Python: {py} | JS/TS: {js} | Rust: {rs}\n\
             - Test dirs: {tests}\n\
             - Entry points: {entries}\n\
             - Test hook: {hook}\n\n\
             ## Dimensions to execute (in order)\n\n\
             1. Input Boundary  2. State Machine Torture  3. Temporal/Timing\n\
             4. AuthN/Z Shadow  5. Data Integrity Cascade  6. Concurrent Load\n\
             7. External Dependency Failure  8. Brownfield Log Mining\n\n\
             ## Safeguards (non-negotiable)\n\n\
             no real data · no secrets · sandboxed execution · no external calls ·\n\
             bounded 30s/flow · no mutation side effects\n\n\
             ## Next step\n\n\
             Load the full skill at `{skill_uri}` (via `skills/get`) for the scenario\n\
             generator, the standard `wde_results.json` schema, and the renderer CLI.",
            name = name,
            p = project_path,
            skill_uri = skill_uri,
            src = recon.source_files,
            py = recon.py,
            js = recon.js,
            rs = recon.rust,
            tests = if recon.test_dirs.is_empty() {
                "none".into()
            } else {
                recon.test_dirs.join(", ")
            },
            entries = if recon.entry_points.is_empty() {
                "none detected".into()
            } else {
                recon.entry_points.join(", ")
            },
            hook = recon.test_hook,
        );

        Ok(CallToolResult::success(vec![ContentBlock::text(body)]))
    }
}

/// Lightweight, read-only reconnaissance of a project directory.
struct Recon {
    source_files: usize,
    py: usize,
    js: usize,
    rust: usize,
    test_dirs: Vec<String>,
    entry_points: Vec<String>,
    test_hook: String,
}

impl WdeServer {
    fn recon(&self, root: &Path) -> Recon {
        let mut recon = Recon {
            source_files: 0,
            py: 0,
            js: 0,
            rust: 0,
            test_dirs: Vec::new(),
            entry_points: Vec::new(),
            test_hook: "none detected — assessment must use dry-run mode".into(),
        };
        self.walk_recon(root, root, &mut recon, 0);
        recon.test_dirs.sort();
        recon.entry_points.sort();
        if !recon.test_dirs.is_empty() {
            recon.test_hook = format!("sandbox candidate: {}", recon.test_dirs.join(", "));
        }
        recon
    }

    fn walk_recon(&self, root: &Path, dir: &Path, recon: &mut Recon, depth: usize) {
        if depth > 3 {
            return;
        }
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || name == "node_modules" || name == "target" {
                continue;
            }
            if path.is_dir() {
                if (name == "tests" || name == "test")
                    && let Ok(rel) = path.strip_prefix(root)
                {
                    recon.test_dirs.push(rel.to_string_lossy().to_string());
                }
                self.walk_recon(root, &path, recon, depth + 1);
                continue;
            }
            match path.extension().and_then(|s| s.to_str()) {
                Some("py") => {
                    recon.py += 1;
                    recon.source_files += 1;
                    if matches!(
                        name.as_str(),
                        "main.py" | "app.py" | "cli.py" | "__main__.py"
                    ) {
                        recon.entry_points.push(name);
                    }
                }
                Some("js") | Some("ts") | Some("tsx") | Some("jsx") => {
                    recon.js += 1;
                    recon.source_files += 1;
                }
                Some("rs") => {
                    recon.rust += 1;
                    recon.source_files += 1;
                    if name == "main.rs" {
                        recon.entry_points.push("src/main.rs".into());
                    }
                }
                _ => {}
            }
        }
    }
}

// ===========================================================================
// ServerHandler — tools (via #[tool_handler]) + skills (manual, SEP-2640)
// ===========================================================================
//
// NOTE: `#[skill_handler]` on this branch expands to `.enable_skills()`, which
// does NOT exist on `ServerCapabilitiesBuilder`. So we implement the skills
// trait methods by hand (matching the compiling `skill_fs_server.rs` example)
// and declare capabilities explicitly in `get_info`.

#[tool_handler]
impl ServerHandler for WdeServer {
    fn get_info(&self) -> InitializeResult {
        let mut caps = ServerCapabilities::builder().enable_tools().build();
        // Advertise the SEP-2640 skills extension. The SDK has no dedicated
        // `skills` field on ServerCapabilities on this branch, so it goes in
        // the generic extension map (SEP-1724).
        let mut ext = rmcp::model::ExtensionCapabilities::new();
        ext.insert(
            "io.modelcontextprotocol/skills".to_string(),
            serde_json::Map::new(),
        );
        caps.extensions = Some(ext);

        InitializeResult::new(caps)
            .with_server_info(Implementation::new(
                "wde-mcp-server",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(
                "Worst-Day-Ever adversarial stress-testing. Call `worst_day_ever_assess` \
                 with a project path, and read the skill at skill://worst-day-ever/SKILL.md.",
            )
    }

    async fn skills_list(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<SkillsListResult, McpError> {
        let mut skills: Vec<SkillEntry> = self.skills.values().cloned().collect();
        skills.sort_by(|a, b| a.uri.cmp(&b.uri));
        Ok(SkillsListResult::new(skills))
    }

    async fn skills_get(
        &self,
        request: SkillsGetRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<SkillsGetResult, McpError> {
        // Accept both the canonical `skill://<path>/SKILL.md` form and the bare
        // `skill://<path>` root form, so an agent that guesses either one works.
        let uri = &request.uri;
        let normalized = if self.skills.contains_key(uri) {
            uri.clone()
        } else {
            format!("{}/SKILL.md", uri.trim_end_matches('/'))
        };
        match self.skills.get(&normalized) {
            Some(skill) => Ok(SkillsGetResult::new(skill.clone())),
            None => Err(McpError::invalid_params(
                format!(
                    "skill not found: {}. Known skills: {}. Use the exact uri from skills/list.",
                    request.uri,
                    self.skills.keys().cloned().collect::<Vec<_>>().join(", ")
                ),
                None,
            )),
        }
    }

    async fn resources_directory_read(
        &self,
        request: skills::ResourcesDirectoryReadRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<skills::ResourcesDirectoryReadResult, McpError> {
        // Accept either the skill ROOT uri (`skill://worst-day-ever`) or the
        // SKILL.md uri (`skill://worst-day-ever/SKILL.md`); the file map is
        // keyed by the root form.
        let root_uri = request
            .uri
            .strip_suffix("/SKILL.md")
            .unwrap_or(&request.uri);
        let files = self.skill_files.get(root_uri);
        let mut children: Vec<Resource> = Vec::new();
        if let Some(files) = files {
            for rel in files.keys() {
                let name = rel.rsplit('/').next().unwrap_or(rel).to_string();
                children.push(Resource::new(format!("skill://{}", rel), name));
            }
            children.sort_by(|a, b| a.uri.cmp(&b.uri));
        }
        let mut result = skills::ResourcesDirectoryReadResult::new(children);
        let supports_cache = context
            .protocol_version()
            .is_some_and(|v| v.as_str() >= ProtocolVersion::V_2026_07_28.as_str());
        if supports_cache {
            result.ttl_ms = Some(600_000);
            result.cache_scope = Some("public".to_string());
        }
        Ok(result)
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, McpError> {
        let rel = request.uri.strip_prefix("skill://").ok_or_else(|| {
            McpError::invalid_params(format!("not a skill URI: {}", request.uri), None)
        })?;
        let bytes = self
            .skill_files
            .values()
            .find_map(|files| files.get(rel))
            .ok_or_else(|| {
                McpError::invalid_params(format!("resource not found: {}", request.uri), None)
            })?;
        let text = String::from_utf8_lossy(bytes).to_string();
        let contents =
            ResourceContents::text(text, request.uri.clone()).with_mime_type("text/markdown");
        Ok(ReadResourceResult::new(vec![contents]).into())
    }
}
