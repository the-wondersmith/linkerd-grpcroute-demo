use std::{fs, path::PathBuf};

/// Read protobuf definitions from `./proto` to generate sources into `./src/gen`.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // List all protobuf definitions.
    let proto_dir = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/proto"));

    let proto_defs = fs::read_dir(&*proto_dir)?
        .flatten()
        .filter_map(|entry| {
            if entry.metadata().is_ok_and(|entry| entry.is_file())
                && entry.path().extension().and_then(|ext| ext.to_str()) == Some("proto")
            {
                Some(entry.path())
            } else {
                None
            }
        })
        .collect::<Vec<PathBuf>>();

    // Generate rust bindings.
    let out_dir = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/src", "/gen"));

    match tonic_build::configure()
        .build_client(true)
        .build_server(true)
        .build_transport(true)
        .out_dir(out_dir)
        .compile(&proto_defs, &[proto_dir])
    {
        Ok(_) => std::process::exit(0),
        Err(error) => {
            eprintln!("{}\n", error.to_string().replace("\\n", "\n").trim_end());
            std::process::exit(1)
        }
    }
}
