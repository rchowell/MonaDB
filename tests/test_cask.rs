
// https://github.com/la10736/rstest


use std::path::PathBuf;

use bytes::Bytes;
use monadb::cask::Cask;
use monadb::Result;

fn tmp_pth() -> PathBuf {
    let tmp_dir= tempfile::tempdir().unwrap();
    let tmp_pth = tmp_dir.path().join("mona.cask");
    tmp_pth
}

#[ignore = "changing cask impl"]
#[test]
fn test_cask_1k() -> Result<()> {
    let path = tmp_pth();
    let mut cask = Cask::new(path)?;
    for _ in 0..1_000 {
        let k: Bytes = b"key".as_slice().into();
        let v: Bytes = b"value".as_slice().into();
        cask.put(0, k, v)?;
    }
    Ok(())
}

#[test]
fn test_cask_10k() {

}

#[test]
fn test_cask_100k() {

}

#[test]
fn test_cask_1m() {

}
