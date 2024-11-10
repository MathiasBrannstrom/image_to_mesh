/// This module contains functions and structures for creating and saving 3D meshes from images.
pub mod contour;

use contour::find_contour_from_transparency_with_offset;
use obj_exporter::{Geometry, ObjSet, Object, Primitive, Shape, TVertex, Vertex};
use image::DynamicImage;
use std::{error::Error, vec};
use rgeometry::{data::Polygon, data::Point};
use std::fs::File;
use std::io::{Read, Write};

/// Represents a 3D mesh with vertices, triangles, UV vertices, and normals.
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub triangles: Vec<Primitive>,
    pub uv_vertices: Vec<TVertex>,
    pub normals: Vec<Vertex>,
}

/// Parameters for creating a mesh, including contour parameters, thickness, and whether to include UVs.
pub struct Params {
    pub contour_params: contour::Params,
    pub thickness: f64,
    pub include_uvs: bool,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            contour_params: contour::Params::default(),
            thickness: 0.05,
            include_uvs: true,
        }
    }
}

pub fn create_mesh_from_image(img: &DynamicImage, params: Params) -> Result<Mesh, Box<dyn Error>> {
    
    let contour = find_contour_from_transparency_with_offset(img, params.contour_params)?;

    let n_points = contour.len();
    
    let polygon = Polygon::new(contour.iter().map(|p| Point::new([p[0], p[1]])).collect()).unwrap();
    
    let front_vertices = contour.iter().map(|p| Vertex{x: (0.5 - p[0]) as f64, y: (0.5 - p[1]) as f64, z: 0.0});
    let back_vertices = contour.iter().map(|p| Vertex{x: (0.5 - p[0]) as f64, y: (0.5 - p[1]) as f64, z: params.thickness});
   
    let triangulation: Vec<(usize, usize, usize)> = rgeometry::algorithms::triangulation::earclip::earclip(&polygon).map(|(p0, p1, p2)| (p0.usize(), p1.usize(), p2.usize())).collect();
    let front_triangles = triangulation.iter()
    .map(|(v0, v1, v2)| triangle_from_indices(*v0, *v2, *v1));

    let back_triangles =  triangulation.iter()
    .map(|(v0, v1, v2)| triangle_from_indices(*v0+n_points, *v1 + n_points, *v2 + n_points));

    let main_triangles = front_triangles.chain(back_triangles);
    let vertices = front_vertices.chain(back_vertices);
    
    let uvs = match params.include_uvs {
        true => {
            let front_uv_vertices = contour.iter().map(|p| TVertex{u: p[0] as f64, v: 1.0- p[1] as f64, w: 0.0});
            let back_uv_vertices = contour.iter().map(|p| TVertex{u: p[0] as f64, v: 1.0 - p[1] as f64, w: 0.0});
            front_uv_vertices.chain(back_uv_vertices).collect()
        }
        false => vec![]
    };

    let main_normals = 
    contour.iter().map(|_| Vertex{x: 0.0, y: 0.0, z: -1.0})
    .chain(contour.iter().map(|_| Vertex{x: 0.0, y: 0.0, z: 1.0}));

    let mut side_triangles:Vec<Primitive> = vec![];
    let mut side_normals:Vec<Vertex> = vec![];

    for i in 0..n_points {
        let prev = if i == 0 {n_points - 1} else {i - 1};
        let next = (i + 1) % n_points;

        side_triangles.push(Primitive::Triangle(
            (i, Some(i), Some(i+2*n_points)), 
            (next+n_points, Some(next+n_points), Some(next+2*n_points)), 
            (i + n_points, Some(i+n_points), Some(i + 2*n_points))));
        side_triangles.push(Primitive::Triangle(
            (i, Some(i), Some(i + 2*n_points)), 
            (next, Some(next), Some(next + 2*n_points)), 
            (next + n_points, Some(next+n_points), Some(next + 2*n_points))));
    
        let v0 = contour[prev];
        let v1 = contour[i];
        let v2 = contour[next];

        let normal_0 = normal_of_line(v0, v1);
        let normal_1 = normal_of_line(v1, v2);

        let normal = [(normal_0[0] + normal_1[0]) / 2.0, (normal_0[1] + normal_1[1]) / 2.0];
        side_normals.push(Vertex{x: normal[0] as f64, y: normal[1] as f64, z: 0.0});
    }

    let mesh = Mesh{
        vertices: vertices.collect(),
        triangles: main_triangles.chain(side_triangles).collect(),
        uv_vertices: uvs.clone(),
        normals: main_normals.into_iter().chain(side_normals).collect(),
    };

    Ok(mesh)
}

fn normal_of_line(v0: [f32; 2], v1: [f32; 2]) -> [f32; 2] {
    let v = [v1[0] - v0[0], v1[1] - v0[1]];
    let len = (v[0] * v[0] + v[1] * v[1]).sqrt();
    [v[0] / len, v[1] / len]
}

/// Creates a mesh from an image and saves it to a file.
///
/// # Arguments
///
/// * `img` - A reference to the image to create the mesh from.
/// * `file_path` - The file path to save the mesh to.
/// * `params` - Parameters for creating the mesh.
///
/// # Returns
///
/// A `Result` which is `Ok` if the mesh was created and saved successfully, or an `Err` containing a boxed error.
pub fn create_and_save_mesh_from_image(
    img: &DynamicImage,
    file_path: &str,
    params: Params,
) -> Result<(), Box<dyn Error>> {
    let mesh = create_mesh_from_image(img, params)?;
    save_mesh_to_file(mesh, file_path)
}

fn triangle_from_indices(v0: usize, v1: usize, v2: usize) -> Primitive {
    Primitive::Triangle(
        (v0, Some(v0), Some(v0)),
        (v1, Some(v1), Some(v1)),
        (v2, Some(v2), Some(v2)),
    )
}

/// Saves a mesh to a OBJ file.
///
/// # Arguments
///
/// * `mesh` - The mesh to save.
/// * `file_path` - The file path to save the mesh to. Has to end with `.obj`.
///
/// # Returns
///
/// A `Result` which is `Ok` if the mesh was saved successfully, or an `Err` containing a boxed error.
pub fn save_mesh_to_file(mesh: Mesh, file_path: &str) -> Result<(), Box<dyn Error>> {
    let shapes = mesh.triangles.iter().map(|triangle| {
        Shape {
            primitive: *triangle,
            groups: vec![],
            smoothing_groups: vec![],
        }
    });

    let geometry = Geometry {
        material_name: Some("material".to_string()),
        shapes: shapes.collect(),
    };

    let obj = Object {
        name: std::path::Path::new(file_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("default")
            .to_string(),
        vertices: mesh.vertices,
        tex_vertices: mesh.uv_vertices,
        normals: mesh.normals,
        geometry: vec![geometry],
    };
    
    let mtl_file_path = file_path.replace(".obj", ".mtl");
    // Extract the filename + extension from the mtl_file_path
    let mtl_filename = std::path::Path::new(&mtl_file_path)
    .file_name()
    .and_then(|s| s.to_str())
    .unwrap_or("material.mtl");

    // Extract the filename + extension from the mtl_file_path
    let png_filename = mtl_filename.replace(".mtl", ".png");


    let obj_set = ObjSet {
        material_library: Some(mtl_file_path.clone()),
        objects: vec![obj],
    };

    let mut mtl_file = File::create(&mtl_file_path)?;
    writeln!(mtl_file, "newmtl material")?;
    writeln!(mtl_file, "map_Kd {}", png_filename)?;

    obj_exporter::export_to_file(&obj_set, file_path).map_err(|e| Box::new(e) as Box<dyn Error>)?;

    // Open the file at file_path and read its contents
    let mut obj_file = File::open(file_path)?;
    let mut obj_contents = String::new();
    obj_file.read_to_string(&mut obj_contents)?;

    

    // Prepend "mtllib {mtl_filename}" to the contents
    let mut new_contents = format!("mtllib {}\n{}", mtl_filename, obj_contents);

    // Add "usemtl material" before the first line that starts with 'f'
    if let Some(pos) = new_contents.find("\nf") {
        let (before, after) = new_contents.split_at(pos + 1);
        new_contents = format!("{}\nusemtl material\n{}", before, after);
    }
    // Write the new contents back to the file
    let mut obj_file = File::create(file_path)?;
    obj_file.write_all(new_contents.as_bytes())?;

    Ok(())
}