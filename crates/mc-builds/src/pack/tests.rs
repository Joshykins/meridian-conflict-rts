//! A build packed into a folder store and installed from it through curl.

use super::*;
use crate::fetch::{file_url, Store};
use crate::versions::{Progress, Versions};
use std::sync::atomic::AtomicBool;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mc-builds-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn identity(build: &str, number: u32) -> Identity {
    Identity {
        build: build.into(),
        number,
        channel: Channel::Playtest,
        commit: "0123456789".into(),
        sim: "00000000000000ff".into(),
        platform: crate::platform().into(),
    }
}

fn stage(dir: &Path, exe: &[u8]) {
    std::fs::create_dir_all(dir.join("data/factions")).unwrap();
    std::fs::write(dir.join(crate::game_exe()), exe).unwrap();
    std::fs::write(dir.join("data/factions/unit.ron"), vec![b'u'; 50_000]).unwrap();
    std::fs::write(dir.join("data/same.ron"), vec![b'u'; 50_000]).unwrap();
}

#[test]
fn packed_builds_install_and_share_their_files() {
    let root = scratch("install");
    let store_dir = root.join("store");
    let key = SigningKey::from_bytes(&[5; 32]);

    stage(&root.join("a"), b"game one");
    let a = pack(
        &root.join("a"),
        &store_dir,
        identity("0.1.0-playtest+aaaaaaaaaa", 1),
        1,
    )
    .unwrap();
    // Two files with the same bytes are one blob.
    assert_eq!(a.new_blobs, 2);
    publish(&store_dir, &a.manifest, &key, true).unwrap();
    assert!(store_dir
        .join(Manifest::channel_path(Channel::Playtest, crate::platform()))
        .is_file());

    stage(&root.join("b"), b"game two");
    let b = pack(
        &root.join("b"),
        &store_dir,
        identity("0.1.0-playtest+bbbbbbbbbb", 2),
        2,
    )
    .unwrap();
    assert_eq!(b.new_blobs, 1, "only the executable changed");

    let store = Store::new(&file_url(&store_dir)).unwrap();
    let versions = Versions::new(&root.join("install"));
    let never = AtomicBool::new(false);
    let mut last = Progress::default();
    versions
        .install(&store, &a.manifest, &mut |p| last = p, &never)
        .unwrap();
    assert_eq!(last.done, last.total);
    assert!(versions.has(&a.manifest.build));
    let exe = std::fs::read(versions.exe(&a.manifest.key())).unwrap();
    assert_eq!(exe, b"game one");

    // The second build downloads only what the first lacks.
    let exe_blob = b
        .manifest
        .files
        .iter()
        .find(|e| e.path == crate::game_exe())
        .unwrap();
    assert_eq!(versions.download_size(&b.manifest), exe_blob.packed);
    versions
        .install(&store, &b.manifest, &mut |_| {}, &never)
        .unwrap();
    assert_eq!(versions.installed().len(), 2);
    assert_eq!(
        versions.installed()[0].build,
        b.manifest.build,
        "newest first"
    );
    assert_eq!(versions.download_size(&b.manifest), 0);

    versions.set_current(&a.manifest).unwrap();
    assert_eq!(
        versions.current(Channel::Playtest).unwrap().build,
        a.manifest.build
    );
    // The newest by last use stays, and so does a build spared by name.
    versions.touch(&b.manifest.key());
    versions.prune(1, &[a.manifest.key()]);
    assert_eq!(versions.installed().len(), 2);
    versions.prune(1, &[]);
    assert_eq!(versions.installed()[0].build, b.manifest.build);
    assert_eq!(versions.installed().len(), 1);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_damaged_blob_is_refused() {
    let root = scratch("damaged");
    let store_dir = root.join("store");
    stage(&root.join("a"), b"game one");
    let a = pack(
        &root.join("a"),
        &store_dir,
        identity("0.1.0-playtest+cccccccccc", 1),
        1,
    )
    .unwrap();
    let exe = a
        .manifest
        .files
        .iter()
        .find(|e| e.path == crate::game_exe())
        .unwrap();
    let other = a
        .manifest
        .files
        .iter()
        .find(|e| e.path != crate::game_exe())
        .unwrap();
    // The executable's blob replaced by another file's.
    std::fs::copy(
        store_dir.join(format!("blobs/{}.zst", other.sha256)),
        store_dir.join(format!("blobs/{}.zst", exe.sha256)),
    )
    .unwrap();
    let store = Store::new(&file_url(&store_dir)).unwrap();
    let versions = Versions::new(&root.join("install"));
    let r = versions.install(&store, &a.manifest, &mut |_| {}, &AtomicBool::new(false));
    assert!(r.is_err());
    assert!(!versions.has(&a.manifest.build));
    let _ = std::fs::remove_dir_all(&root);
}
