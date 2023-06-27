use std::collections::HashMap;
use std::env;
use std::fmt::Display;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use serde::Deserialize;

#[derive(Deserialize)]
struct ObjectData {
    texture: Option<String>,
    default_z_layer: Option<i8>,
    default_z_order: Option<i16>,
    default_base_color_channel: Option<u64>,
    default_detail_color_channel: Option<u64>,
    color_type: Option<String>,
    swap_base_detail: Option<bool>,
    opacity: Option<f32>,
    children: Option<Vec<Child>>,
}

#[derive(Deserialize)]
struct Child {
    texture: String,
    x: f32,
    y: f32,
    z: i16,
    rot: f32,
    anchor_x: f32,
    anchor_y: f32,
    scale_x: f32,
    scale_y: f32,
    flip_x: bool,
    flip_y: bool,
    color_type: Option<String>,
    opacity: Option<f32>,
    children: Option<Vec<Child>>,
}

fn main() {
    println!("cargo:rerun-if-changed=gdclone.manifest");
    println!("cargo:rerun-if-changed=gdclone.rc");

    embed_resource::compile("gdclone.rc", embed_resource::NONE);

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
    match object_id {\n"
                .as_bytes(),
        )
        .unwrap();
    for (k, v) in hashmap {
        write_object(k, v, 2, &mut dest_file);
    }
    dest_file
        .write_all("        _ => ObjectDefaultData::default(),\n".as_bytes())
        .unwrap();
    dest_file.write_all("    }".as_bytes()).unwrap();
    dest_file.write_all("}".as_bytes()).unwrap();
}

fn write_object(id: u64, object_data: ObjectData, indent_len: usize, file: &mut File) {
    let indent = "    ".repeat(indent_len);
    file.write_all(
        (indent.clone() + format!("{} => ObjectDefaultData {{\n", id).as_str()).as_bytes(),
    )
    .unwrap();

    let mut has_values = [
        false, false, false, false, false, false, false, false, false,
    ];
    if let Some(texture) = object_data.texture {
        write_value_string("texture", texture.as_str(), indent_len + 1, file);
        has_values[0] = true;
    }
    if let Some(default_z_layer) = object_data.default_z_layer {
        write_value("default_z_layer", default_z_layer, indent_len + 1, file);
        has_values[1] = true;
    }
    if let Some(default_z_order) = object_data.default_z_order {
        write_value("default_z_order", default_z_order, indent_len + 1, file);
        has_values[2] = true;
    }
    if let Some(default_base_color_channel) = object_data.default_base_color_channel {
        write_value(
            "default_base_color_channel",
            default_base_color_channel,
            indent_len + 1,
            file,
        );
        has_values[3] = true;
    }
    if let Some(default_detail_color_channel) = object_data.default_detail_color_channel {
        write_value(
            "default_detail_color_channel",
            default_detail_color_channel,
            indent_len + 1,
            file,
        );
        has_values[4] = true;
    }
    if let Some(color_type) = object_data.color_type {
        write_value_color_type("color_type", color_type, indent_len + 1, file);
        has_values[5] = true;
    }
    if let Some(swap_base_detail) = object_data.swap_base_detail {
        write_value("swap_base_detail", swap_base_detail, indent_len + 1, file);
        has_values[6] = true;
    }
    if let Some(opacity) = object_data.opacity {
        write_value_f32("opacity", opacity, indent_len + 1, file);
        has_values[7] = true;
    }
    if let Some(children) = object_data.children {
        file.write_all(("    ".repeat(indent_len + 1) + "children: vec![\n").as_bytes())
            .unwrap();
        for child in children {
            write_child(child, indent_len + 2, file);
        }
        file.write_all(("    ".repeat(indent_len + 1) + "],\n").as_bytes())
            .unwrap();
        has_values[8] = true;
    }
    if has_values != [true, true, true, true, true, true, true, true, true] {
        file.write_all(("    ".repeat(indent_len + 1) + "..default()\n").as_bytes())
            .unwrap();
    }
    file.write_all((indent + "},\n").as_bytes()).unwrap();
}

fn write_child(child: Child, indent_len: usize, file: &mut File) {
    let indent = "    ".repeat(indent_len);
    file.write_all((indent.clone() + "ObjectChild {\n").as_bytes())
        .unwrap();
    write_value_string("texture", child.texture.as_str(), indent_len + 1, file);
    let mut has_values = [
        false, false, false, false, false, false, false, false, false,
    ];
    if child.x != 0. || child.y != 0. || child.z != 0 {
        write_value_vec3(
            "offset",
            child.x,
            child.y,
            child.z as f32,
            indent_len + 1,
            file,
        );
        has_values[0] = true;
    }
    if child.rot != 0. {
        write_value_f32("rotation", child.rot, indent_len + 1, file);
        has_values[1] = true;
    }
    if child.anchor_x != 0. || child.anchor_y != 0. {
        write_value_vec2(
            "anchor",
            child.anchor_x,
            child.anchor_y,
            indent_len + 1,
            file,
        );
        has_values[2] = true;
    }
    if child.scale_x != 1. || child.scale_y != 1. {
        write_value_vec2("scale", child.scale_x, child.scale_y, indent_len + 1, file);
        has_values[3] = true;
    }
    if child.flip_x {
        write_value("flip_x", child.flip_x, indent_len + 1, file);
        has_values[4] = true;
    }
    if child.flip_y {
        write_value("flip_y", child.flip_y, indent_len + 1, file);
        has_values[5] = true;
    }
    if let Some(color_type) = child.color_type {
        write_value_color_type("color_type", color_type, indent_len + 1, file);
        has_values[6] = true;
    }
    if let Some(opacity) = child.opacity {
        write_value_f32("opacity", opacity, indent_len + 1, file);
        has_values[7] = true;
    }
    if let Some(children) = child.children {
        file.write_all(("    ".repeat(indent_len + 1) + "children: vec![\n").as_bytes())
            .unwrap();
        for child in children {
            write_child(child, indent_len + 2, file);
        }
        file.write_all(("    ".repeat(indent_len + 1) + "],\n").as_bytes())
            .unwrap();
        has_values[8] = true;
    }
    if has_values != [true, true, true, true, true, true, true, true, true] {
        file.write_all(("    ".repeat(indent_len + 1) + "..default()\n").as_bytes())
            .unwrap();
    }
    file.write_all((indent + "},\n").as_bytes()).unwrap();
}

fn write_value_string(name: &str, value: &str, indent_len: usize, file: &mut File) {
    let indent = "    ".repeat(indent_len);
    file.write_all(
        (indent + format!("{}: \"{}\".to_string(),\n", name, value).as_str()).as_bytes(),
    )
    .unwrap();
}

fn write_value_color_type(name: &str, value: String, indent_len: usize, file: &mut File) {
    let indent = "    ".repeat(indent_len);
    file.write_all(
        (indent + format!("{}: ObjectColorType::{},\n", name, value).as_str()).as_bytes(),
    )
    .unwrap();
}

fn write_value_vec2(name: &str, x: f32, y: f32, indent_len: usize, file: &mut File) {
    let indent = "    ".repeat(indent_len);
    file.write_all(
        (indent
            + format!(
                "{}: Vec2::new({}, {}),\n",
                name,
                f32_writable(x),
                f32_writable(y)
            )
            .as_str())
        .as_bytes(),
    )
    .unwrap();
}

fn write_value_vec3(name: &str, x: f32, y: f32, z: f32, indent_len: usize, file: &mut File) {
    let indent = "    ".repeat(indent_len);
    file.write_all(
        (indent
            + format!(
                "{}: Vec3::new({}, {}, {}),\n",
                name,
                f32_writable(x),
                f32_writable(y),
                f32_writable(z)
            )
            .as_str())
        .as_bytes(),
    )
    .unwrap();
}

fn write_value_f32(name: &str, value: f32, indent_len: usize, file: &mut File) {
    let indent = "    ".repeat(indent_len);
    file.write_all((indent + format!("{}: {},\n", name, f32_writable(value)).as_str()).as_bytes())
        .unwrap();
}

fn f32_writable(value: f32) -> String {
    if value.fract() == 0. {
        return format!("{}.", value);
    }
    format!("{}", value)
}

fn write_value<T>(name: &str, value: T, indent_len: usize, file: &mut File)
where
    T: Display,
{
    let indent = "    ".repeat(indent_len);
    file.write_all((indent + format!("{}: {},\n", name, value).as_str()).as_bytes())
        .unwrap();
}
