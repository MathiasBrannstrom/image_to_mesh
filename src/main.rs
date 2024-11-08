use obj_exporter::{export_to_file, Geometry, ObjSet, Object, Primitive, Shape, Vertex};
use std::error::Error;

fn main() {
    println!("Hello, world!");

    save_mesh_to_file("C:\\tmp\\test.obj").unwrap();
}


fn get_points_from_image(image_path: &str) -> Result<Vec<(f64, f64)>, Box<dyn Error>> {
    let img = image::open(image_path)?;
    let (width, height) = img.dimensions();
    let mut points = vec![];

    for y in 0..height {
        for x in 0..width {
            let pixel = img.get_pixel(x, y);
            let (r, g, b, a) = pixel.channels4();
            let point = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
            points.push(point);
        }
    }

    Ok(points)
}


fn save_mesh_to_file(file_path: &str) -> Result<(), Box<dyn Error>> {
    let vertices = vec![
        Vertex { x: 0.0, y: 0.0, z: 0.0 }, 
        Vertex { x: 1.0, y: 0.0, z: 0.0 }, 
        Vertex { x: 1.0, y: 1.0, z: 0.0 }, 
        Vertex { x: 0.0, y: 1.0, z: 0.0 }
    ];
    
    let triangles = vec![
        Primitive::Triangle((0, None, None), (1, None, None), (2, None, None)), 
        Primitive::Triangle((0, None, None), (2, None, None), (3, None, None))
    ];

    let shapes = triangles.into_iter().map(|triangle| {
        Shape {
            primitive: triangle,
            groups: vec![],
            smoothing_groups: vec![],
        }
    });

    let geometry = Geometry {
        material_name: None,
        shapes: shapes.collect(),
    };

    let mesh = Object {
        name: "test".to_string(), 
        vertices: vertices, 
        tex_vertices: vec![], 
        normals: vec![], 
        geometry: vec![geometry]
    };

    let obj_set = ObjSet {
        material_library: None,
        objects: vec![mesh]
    };

    export_to_file(&obj_set, file_path)?;

    Ok(())
}