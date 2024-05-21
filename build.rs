use anyhow::Context;
use std::collections::HashSet;

/// Ensure the protobuf definitions from `./proto` are in sync with their upstream source
fn main() -> anyhow::Result<()> {
    // List all protobuf definitions.
    let proto_dir = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/proto"));
    let proto_cache = std::path::PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/target",
        "/proto-cache"
    ));

    if !proto_cache.exists() {
        std::fs::create_dir_all(proto_cache.as_path())?;
    }

    let mut cached_defs = proto_cache
        .read_dir()?
        .flatten()
        .filter_map(|entry| {
            if entry.metadata().is_ok_and(|entry| entry.is_file())
                && entry.path().extension().and_then(|ext| ext.to_str()) == Some("proto")
            {
                Some(entry.file_name())
            } else {
                None
            }
        })
        .collect::<HashSet<_>>();

    let proto_defs = std::fs::read_dir(&*proto_dir)?
        .flatten()
        .filter_map(|entry| {
            if entry.metadata().is_ok_and(|entry| entry.is_file())
                && entry.path().extension().and_then(|ext| ext.to_str()) == Some("proto")
            {
                Some(entry.file_name())
            } else {
                None
            }
        })
        .collect::<HashSet<_>>();

    if cached_defs.is_empty() || proto_defs.is_empty() || cached_defs != proto_defs {
        let agent = ureq::AgentBuilder::new()
            .timeout_read(std::time::Duration::from_secs(5))
            .timeout_write(std::time::Duration::from_secs(5))
            .build();

        agent
            .get("https://api.github.com/repos/BuoyantIO/emojivoto/contents/proto")
            .set("Accept", "application/vnd.github.object+json")
            .set("X-GitHub-Api-Version", "2022-11-28")
            .call()?
            .into_json::<ureq::serde_json::Value>()
            .and_then(|data| {
                if let ureq::serde_json::Value::Object(mut data) = data {
                    if let Some(ureq::serde_json::Value::Array(entries)) = data.remove("entries") {
                        Ok(entries)
                    } else {
                        Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            anyhow::Error::msg(format!("unexpected data type returned: {data:?}")),
                        ))
                    }
                } else {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        anyhow::Error::msg(format!("unexpected data type returned: {data:?}")),
                    ))
                }
            })
            .map(|entries| {
                entries
                    .into_iter()
                    .filter_map(|entry| {
                        if let ureq::serde_json::Value::Object(mut entry) = entry {
                            match (entry.remove("name"), entry.remove("download_url")) {
                                (Some(name), Some(url)) if name.is_string() && url.is_string() => {
                                    let (url, name) = (
                                        url.to_string().replace('"', ""),
                                        name.to_string().replace('"', ""),
                                    );

                                    Some((
                                        name.to_string(),
                                        agent
                                            .get(url.as_str())
                                            .call()
                                            .unwrap()
                                            .into_string()
                                            .unwrap(),
                                    ))
                                }
                                _ => None,
                            }
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<(String, String)>>()
            })?
            .into_iter()
            .for_each(|(name, data)| {
                let file = proto_cache.join(name);
                std::fs::write(&file, data)
                    .context(format!("file={:?}", &file))
                    .unwrap();
                cached_defs.insert(file.file_name().unwrap().to_os_string());
            });
    };

    fn should_overwrite(left: &std::path::PathBuf, right: &std::path::PathBuf) -> bool {
        if !(left.exists() && right.exists()) {
            true
        } else {
            sha256::digest(std::fs::read_to_string(left).unwrap())
                != sha256::digest(std::fs::read_to_string(right).unwrap())
        }
    }

    let mut regenerate = false;

    for def in proto_defs.union(&cached_defs) {
        let (source, target) = (proto_cache.join(def), proto_dir.join(def));

        if should_overwrite(&source, &target) {
            std::fs::write(target, std::fs::read_to_string(source)?)?;

            regenerate = true;
            // Tell Cargo that if one of the proto defs changes, to rerun this build script.
            println!("cargo::rerun-if-changed=proto/{}", def.to_str().unwrap());
        }
    }

    if regenerate {
        Err(anyhow::Error::msg(
            "protobuf definitions have changed! please re-run: cargo run --example=generate-types",
        ))
    } else {
        Ok(())
    }
}
