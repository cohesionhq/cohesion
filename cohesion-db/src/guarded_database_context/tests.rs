use crate::test_support::context;
use cohesion_core::model::{Dialect, IntelligenceEndpointPreset};

use crate::{
    CreateIntelligenceEndpointCommand, CreateWorkspaceCommand, DatabaseEventTrace,
    ListWorkspacesCommand, WriteArea,
};

fn create(title: &str) -> CreateWorkspaceCommand {
    CreateWorkspaceCommand {
        title: title.to_string(),
        emoji: None,
    }
}

#[test]
fn read_only_guard_refuses_writes() {
    let guard = context().read_only();
    assert!(
        guard
            .send_write_blocking_traced(create("w"), DatabaseEventTrace::default())
            .is_none()
    );
    assert!(
        guard
            .send_read_blocking(ListWorkspacesCommand::default())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn read_only_narrows_a_read_write_guard() {
    let guard = context().read_write();
    let narrowed = guard.read_only();
    assert!(
        narrowed
            .send_write_blocking_traced(create("w"), DatabaseEventTrace::default())
            .is_none()
    );
    assert!(
        guard
            .send_write_blocking_traced(create("w"), DatabaseEventTrace::default())
            .is_some()
    );
}

fn endpoint() -> CreateIntelligenceEndpointCommand {
    CreateIntelligenceEndpointCommand {
        name: None,
        api_key: None,
        usage_url: None,
        cache_minutes: None,
        long_cache: None,
        preset: IntelligenceEndpointPreset::Custom {
            provider_identifier: "ollama".to_string(),
            base_url: "http://localhost:11434/v1".to_string(),
            dialect: Dialect::ChatCompletions,
        },
    }
}

#[test]
fn a_guard_writes_only_its_areas() {
    let settings = context().writing([WriteArea::IntelligenceSettings]);
    assert!(
        settings
            .send_write_blocking_traced(create("w"), DatabaseEventTrace::default())
            .is_none()
    );
    assert!(
        settings
            .send_write_blocking_traced(endpoint(), DatabaseEventTrace::default())
            .is_some()
    );
    let data = context().writing([WriteArea::UserData]);
    assert!(
        data.send_write_blocking_traced(endpoint(), DatabaseEventTrace::default())
            .is_none()
    );
}
