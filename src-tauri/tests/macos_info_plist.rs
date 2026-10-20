#[test]
fn bundled_macos_info_plist_declares_documents_usage() {
    let plist = include_str!("../Info.plist");
    for key in [
        "NSDocumentsFolderUsageDescription",
        "NSDesktopFolderUsageDescription",
        "NSDownloadsFolderUsageDescription",
        "NSRemovableVolumesUsageDescription",
        "NSNetworkVolumesUsageDescription",
    ] {
        assert!(
            plist.contains(key),
            "src-tauri/Info.plist must declare {key} so macOS can persist Files and Folders grants"
        );
    }
}

#[test]
fn macos_entitlements_keep_app_unsandboxed() {
    let entitlements = include_str!("../Entitlements.plist");
    assert!(
        entitlements.contains("com.apple.security.app-sandbox"),
        "explicit sandbox key is required so the app is not accidentally confined"
    );
    assert!(
        entitlements.contains("<false/>"),
        "debugging needs an unsandboxed macOS app"
    );
    assert!(entitlements.contains("com.apple.security.files.user-selected.read-write"));
    assert!(entitlements.contains("com.apple.security.files.downloads.read-write"));
}
