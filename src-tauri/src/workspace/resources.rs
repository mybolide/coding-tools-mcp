#[cfg(test)]
mod tests {
    use super::{
        validate_service_start, validate_workspace_resources, WorkspaceService,
    };
    use crate::workspace::WorkspaceProfile;

    fn profile(name: &str, mcp_port: u16, actions_port: u16) -> WorkspaceProfile {
        let mut profile = WorkspaceProfile::new(format!("C:/workspace/{name}"), Some(name.into()));
        profile.runtime.local_port = mcp_port;
        profile.actions.local_port = actions_port;
        profile.tunnel.tunnel_type = "frp".into();
        profile.tunnel.frp_subdomain = format!("{name}-mcp");
        profile.actions.tunnel_type = "frp".into();
        profile.actions.frp_subdomain = format!("{name}-actions");
        profile
    }

    #[test]
    fn rejects_duplicate_mcp_port_across_workspaces_before_start() {
        let owner = profile("owner", 28_766, 8_787);
        let target = profile("target", 28_766, 8_788);

        let error = validate_service_start(
            &[owner.clone(), target.clone()],
            &target.id,
            WorkspaceService::Mcp,
        )
        .unwrap_err();

        let message = error.to_string();
        assert!(message.contains("28766"));
        assert!(message.contains(&owner.name));
        assert!(message.contains("MCP"));
    }

    #[test]
    fn rejects_port_shared_by_mcp_and_another_workspace_actions() {
        let mut owner = profile("owner", 28_765, 8_787);
        owner.actions.local_port = 28_766;
        let target = profile("target", 28_766, 8_788);

        let error = validate_service_start(
            &[owner.clone(), target.clone()],
            &target.id,
            WorkspaceService::Mcp,
        )
        .unwrap_err();

        let message = error.to_string();
        assert!(message.contains("28766"));
        assert!(message.contains(&owner.name));
        assert!(message.contains("Actions"));
    }

    #[test]
    fn rejects_duplicate_ports_between_services_in_one_workspace() {
        let candidate = profile("target", 28_766, 28_766);

        let error = validate_workspace_resources(&[], &candidate).unwrap_err();

        let message = error.to_string();
        assert!(message.contains("28766"));
        assert!(message.contains("MCP"));
        assert!(message.contains("Actions"));
    }

    #[test]
    fn allows_a_workspace_to_keep_its_own_ports() {
        let original = profile("target", 28_766, 8_787);
        let updated = original.clone();

        assert!(validate_workspace_resources(&[original], &updated).is_ok());
    }

    #[test]
    fn unrelated_update_is_not_blocked_by_legacy_duplicates() {
        let first = profile("first", 28_766, 8_787);
        let second = profile("second", 28_766, 8_788);
        let candidate = profile("candidate", 28_769, 8_789);

        assert!(validate_workspace_resources(&[first, second], &candidate).is_ok());
    }

    #[test]
    fn start_is_blocked_when_target_participates_in_legacy_duplicate() {
        let first = profile("first", 28_766, 8_787);
        let target = profile("target", 28_766, 8_788);

        let error = validate_service_start(
            &[first, target.clone()],
            &target.id,
            WorkspaceService::Mcp,
        )
        .unwrap_err();

        assert!(error.to_string().contains("端口"));
    }

    #[test]
    fn rejects_duplicate_subdomains_with_owner_details() {
        let owner = profile("owner", 28_765, 8_787);
        let mut candidate = profile("target", 28_766, 8_788);
        candidate.tunnel.frp_subdomain = owner.tunnel.frp_subdomain.clone();

        let error = validate_workspace_resources(&[owner.clone()], &candidate).unwrap_err();

        let message = error.to_string();
        assert!(message.contains(&owner.tunnel.frp_subdomain));
        assert!(message.contains(&owner.name));
        assert!(message.contains("MCP"));
    }

    #[test]
    fn ignores_blank_subdomain_claims() {
        let mut first = profile("first", 28_765, 8_787);
        let mut second = profile("second", 28_766, 8_788);
        first.tunnel.frp_subdomain.clear();
        second.tunnel.frp_subdomain.clear();

        assert!(validate_workspace_resources(&[first], &second).is_ok());
    }
}
