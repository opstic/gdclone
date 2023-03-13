use serde::Deserialize;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

#[derive(Deserialize)]
struct ObjectData {
    texture_name: Option<String>,
    default_z_layer: Option<i8>,
    default_z_order: Option<i16>,
    childrens: Option<Vec<Children>>,
}

#[derive(Deserialize)]
struct Children {
    texture_name: String,
    x: f32,
    y: f32,
    z: i16,
    rot: f32,
}

fn main() {
    println!("cargo:rerun-if-changed=assets/data/object.json");
    let mut object_json = File::open("assets/data/object.json").unwrap();
    let mut bytes = Vec::new();
    if let Ok(metadata) = object_json.metadata() {
        bytes.reserve(metadata.len() as usize);
    }
    object_json.read_to_end(&mut bytes).unwrap();
    let hashmap: HashMap<u64, ObjectData> = serde_json::de::from_slice(&bytes).unwrap();
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let mut dest_file = File::create(Path::new(&out_dir).join("generated_object.rs")).unwrap();
    dest_file
        .write_all(
            "fn object_handler(object_id: u64) -> ObjectDefaultData {
    let mut data = ObjectDefaultData::default();
    match object_id {\n"
                .as_bytes(),
        )
        .unwrap();
    for (k, v) in hashmap {
        dest_file
            .write_all(format!("        {} => {{\n", k).as_bytes())
            .unwrap();
        if let Some(texture_name) = v.texture_name {
            dest_file
                .write_all(
                    format!(
                        "            data.texture_name = \"{}\".to_string();\n",
                        texture_name
                    )
                    .as_bytes(),
                )
                .unwrap();
        }
        if let Some(default_z_layer) = v.default_z_layer {
            dest_file
                .write_all(
                    format!("            data.default_z_layer = {};\n", default_z_layer).as_bytes(),
                )
                .unwrap();
        }
        if let Some(default_z_order) = v.default_z_order {
            dest_file
                .write_all(
                    format!("            data.default_z_order = {};\n", default_z_order).as_bytes(),
                )
                .unwrap();
        }
        if let Some(childrens) = v.childrens {
            dest_file
                .write_all("            data.childrens = vec![\n".as_bytes())
                .unwrap();
            for child in childrens {
                dest_file
                    .write_all("                Children {\n".as_bytes())
                    .unwrap();
                dest_file
                    .write_all(
                        format!(
                            "                    texture_name: \"{}\".to_string(),\n",
                            child.texture_name
                        )
                        .as_bytes(),
                    )
                    .unwrap();
                dest_file
                    .write_all(format!("                    x: {} as f32,\n", child.x).as_bytes())
                    .unwrap();
                dest_file
                    .write_all(format!("                    y: {} as f32,\n", child.y).as_bytes())
                    .unwrap();
                dest_file
                    .write_all(format!("                    z: {},\n", child.z).as_bytes())
                    .unwrap();
                dest_file
                    .write_all(
                        format!("                    rot: {} as f32,\n", child.rot).as_bytes(),
                    )
                    .unwrap();
                dest_file
                    .write_all("                },\n".as_bytes())
                    .unwrap();
            }
            dest_file.write_all("            ];\n".as_bytes()).unwrap();
        }
        dest_file.write_all("        }\n".as_bytes()).unwrap();
    }
    dest_file.write_all("        _ => {}\n".as_bytes()).unwrap();
    dest_file.write_all("    }\n".as_bytes()).unwrap();
    dest_file.write_all("    data\n".as_bytes()).unwrap();
    dest_file.write_all("}".as_bytes()).unwrap();
}
