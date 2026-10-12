//! Regression guards for the agent-drive path.
//!
//! An agent discovers a skill via `skills/list`, then must be able to read the
//! skill's *body* — the instructions it is supposed to follow. `skills/get`
//! returns frontmatter only, so the body is reachable exclusively through
//! `resources/read` on the skill's `SKILL.md` URI. That path once returned
//! "resource not found" because `SKILL.md` was excluded from the resource map,
//! which silently made the server unusable to an agent that followed its own
//! instructions. These tests exist so that cannot regress silently again.

use wde_mcp_server::WdeServer;

fn skill_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("skills")
}

#[test]
fn skill_md_is_advertised_in_resources() {
    let server = WdeServer::from_root(skill_root()).expect("load skills");
    let skills = server.list_skills();
    assert_eq!(skills.len(), 1, "expected exactly one skill");

    let entry = &skills[0];
    let resources = entry
        .resources
        .as_ref()
        .expect("skill should advertise resources");
    let uris: Vec<String> = match resources {
        rmcp::model::SkillResources::FileList(files) => {
            files.iter().map(|f| f.uri.clone()).collect()
        }
        other => panic!("expected a static file list, got {other:?}"),
    };

    assert!(
        uris.iter().any(|u| u.ends_with("/SKILL.md")),
        "SKILL.md must be in the advertised resource list (spec: resources MUST be complete); got {uris:?}"
    );
}

/// The regression that mattered: reading the skill body must work.
#[test]
fn skill_body_is_readable_via_resource_uri() {
    let server = WdeServer::from_root(skill_root()).expect("load skills");
    let uri = "skill://worst-day-ever/SKILL.md";

    let bytes = server.read_skill_file(uri).unwrap_or_else(|| {
        panic!("{uri} must be readable — it is the only path to the skill body")
    });

    let text = String::from_utf8_lossy(bytes);
    assert!(
        text.contains("# Worst-Day-Ever"),
        "the skill body (not just frontmatter) must be returned"
    );
}

/// An agent may guess either the canonical `.../SKILL.md` form or the bare root.
#[test]
fn both_uri_forms_resolve_to_the_same_skill() {
    let server = WdeServer::from_root(skill_root()).expect("load skills");

    let canonical = server.get_skill("skill://worst-day-ever/SKILL.md");
    let bare = server.get_skill("skill://worst-day-ever");

    assert!(canonical.is_some(), "canonical form must resolve");
    assert!(bare.is_some(), "bare root form must also resolve");
    assert_eq!(
        canonical.unwrap().uri,
        bare.unwrap().uri,
        "both forms must resolve to the same skill"
    );
}

/// Every advertised resource must actually be readable — an advertised-but-
/// unreadable resource is worse than an absent one, because it looks complete.
#[test]
fn every_advertised_resource_is_readable() {
    let server = WdeServer::from_root(skill_root()).expect("load skills");
    let skills = server.list_skills();
    let resources = skills[0].resources.as_ref().expect("resources");

    let uris: Vec<String> = match resources {
        rmcp::model::SkillResources::FileList(files) => {
            files.iter().map(|f| f.uri.clone()).collect()
        }
        other => panic!("expected a static file list, got {other:?}"),
    };

    for uri in &uris {
        assert!(
            server.read_skill_file(uri).is_some(),
            "advertised resource {uri} is not readable"
        );
    }
}

/// SEP-2640 conformance: literal-block chomping must follow the YAML 1.2 spec
/// so the frontmatter advertised in `skills/list` matches what a standard YAML
/// parser (e.g. the `yaml` npm package used by MCP Inspector) would produce
/// from the served `SKILL.md`. A bare `|` keeps a trailing newline. This
/// regressed with `serde_yaml`, which silently dropped it.
#[test]
fn literal_block_chomping_is_spec_compliant() {
    let server = WdeServer::from_root(skill_root()).expect("load skills");
    let entry = server.get_skill("skill://worst-day-ever").expect("skill");

    let description = entry
        .frontmatter
        .get("description")
        .and_then(serde_json::Value::as_str)
        .expect("description must be a string");

    assert!(
        description.ends_with("debug sessions.\n"),
        "bare `|` must keep the trailing newline; got tail {description:?}"
    );

    // Reference-parse the served file's frontmatter with yaml-rust2 (the same
    // spec-compliant engine the server uses) and require identical output —
    // i.e. the served value comes from parsing the served bytes, not from any
    // post-processing that could diverge.
    let raw = String::from_utf8_lossy(
        server
            .read_skill_file("skill://worst-day-ever/SKILL.md")
            .expect("SKILL.md readable"),
    );
    let fm = raw
        .strip_prefix("---\n")
        .and_then(|r| r.split("\n---").next());
    let parsed = yaml_rust2::YamlLoader::load_from_str(fm.expect("delimited frontmatter"))
        .expect("frontmatter must parse");
    let reference = parsed[0]["description"]
        .as_str()
        .expect("reference description must be a string");
    assert_eq!(
        description, reference,
        "served description must equal a plain YAML parse of the served bytes"
    );
}
