#![recursion_limit = "256"]

use obj_exporter::{Geometry, ObjSet, Object, Primitive, Shape, TVertex, Vertex};
use image::{GenericImageView, GrayImage, ImageBuffer, Luma, Pixel};
use sdfer::{Image2d, Unorm8};
use std::{error::Error, f32::consts::PI};
use rgeometry::{data::Polygon, data::Point};

fn main() {
    println!("Hello, world!");

    create_mesh_from_image("C:\\tmp\\simple_shape.png").unwrap();
}

fn sub(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

fn normalize(v: [f32; 2]) -> [f32; 2] {
    let norm = (v[0] * v[0] + v[1] * v[1]).sqrt();
    [v[0] / norm, v[1] / norm]
}

fn create_mesh_from_image(image_path: &str) -> Result<(), Box<dyn Error>> {
    let img = image::open(image_path)?;
    let (width, height) = img.dimensions();

    let mut imgbuf = image::GrayImage::new(width, height);    

    for (x, y, pixel) in imgbuf.enumerate_pixels_mut() {
        *pixel = Luma([img.get_pixel(x, y).channels()[3]]);
    }

    let mut sdf = sdf_image(width, height, &imgbuf);

    let biggest_contour = find_contour(&sdf, 40u8);

    let biggest_contour = smooth_contour(biggest_contour,10);

    for point in biggest_contour.iter() {
        sdf.put_pixel(point[0] as u32, point[1] as u32, Luma([255]));
    }    
    sdf.save("C:\\tmp\\testsdf.png")?;
    
    let (f_width, f_height) = (width as f32, height as f32);

    let scaled_points = biggest_contour.iter().map(|p| [p[0] /f_width, p[1] / f_height]).collect::<Vec<[f32; 2]>>();

    // let scaled_points = remove_points_with_angle(scaled_points, n_points, PI/8.0);
    let n_points = scaled_points.len();
    
    println!("Points after deletion: {}", n_points);
    let polygon = Polygon::new(scaled_points.iter().map(|p| Point::new([p[0], p[1]])).collect()).unwrap();
    
    let front_vertices = scaled_points.iter().map(|p| Vertex{x: (f_width - p[0] - 1.0) as f64, y: (f_height - p[1]-1.0) as f64, z: 0.0});
    let back_vertices = scaled_points.iter().map(|p| Vertex{x: (f_width - p[0] - 1.0) as f64, y: (f_height - p[1]-1.0) as f64, z: 0.05});
    
    let front_uv_vertices = scaled_points.iter().map(|p| TVertex{u: p[0] as f64, v: p[1] as f64, w: 0.0});
    let back_uv_vertices = scaled_points.iter().map(|p| TVertex{u: p[0] as f64, v: p[1] as f64, w: 0.0});
    
    let triangulation: Vec<(usize, usize, usize)> = rgeometry::algorithms::triangulation::earclip::earclip(&polygon).map(|(p0, p1, p2)| (p0.usize(), p1.usize(), p2.usize())).collect();
    let front_triangles = triangulation.iter()
    .map(|(v0, v1, v2)| triangle_from_indices(*v0, *v2, *v1, 0));

    let back_triangles =  triangulation.iter()
    .map(|(v0, v1, v2)| triangle_from_indices(*v0, *v1, *v2, n_points));


    let mut side_triangles:Vec<Primitive> = vec![];

    for i in 0..n_points {
        let next = (i + 1) % n_points;
        side_triangles.push(triangle_from_indices(i, next + n_points, i + n_points, 0));
        side_triangles.push(triangle_from_indices(i, next, next + n_points, 0));
    }

    let all_triangles = front_triangles.chain(back_triangles).chain(side_triangles.into_iter());
    let all_vertices = front_vertices.chain(back_vertices);
    let all_uvs = front_uv_vertices.chain(back_uv_vertices);
    save_mesh_to_file(all_vertices, all_triangles, all_uvs, "C:\\tmp\\outlineMesh.obj")?;

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LookDirection {
    Right,
    Down,
    Left,
    Up
}

fn smooth_contour(contour_points: Vec<[f32; 2]>, iterations: u32) -> Vec<[f32; 2]> {
    let mut smoothed_points:Vec<[f32; 2]> = contour_points;

    for _ in 0..iterations {
        
        let mut curr_smoothed_points:Vec<[f32; 2]> = vec![];

        for i in 0..smoothed_points.len() {
            let prev = smoothed_points[(i + smoothed_points.len() - 1) % smoothed_points.len()];
            let current = smoothed_points[i];
            let next = smoothed_points[(i + 1) % smoothed_points.len()];
    
            let smoothed = [(prev[0] + current[0] + next[0]) / 3.0, (prev[1] + current[1] + next[1]) / 3.0];
            curr_smoothed_points.push(smoothed);
        }
        smoothed_points = curr_smoothed_points;
    }

    

    smoothed_points
}

fn find_contour(image: &GrayImage, threshold: u8) -> Vec<[f32; 2]> {
    

    // Find a starting point
    let mut start_point:Option<[u32; 2]> = None;

    for (x, y, pixel) in image.enumerate_pixels() {
        // As we're looking below, skip the last row
        if y == image.height() - 1 { continue };
        

        if pixel[0] <= threshold && image.get_pixel(x, y + 1)[0] > threshold {
            start_point = Some([x, y]);
            break;
        }
    }

    if start_point.is_none() {
        return vec![]; // Maybe return an error instead
    }

    let start_point = start_point.unwrap();
    let mut contour_points:Vec<[f32; 2]> = vec![];

    let mut current_direction = LookDirection::Right;
    let mut current_point = start_point;

    loop{

        // When we come back to the starting point, we're done
        if contour_points.len() > 0 && current_point==start_point { break;}

        let (x, y) = (current_point[0], current_point[1]);

        let comparison_point = match current_direction {
            LookDirection::Right => [x, y + 1],
            LookDirection::Down => [x-1, y],
            LookDirection::Left => [x, y-1],
            LookDirection::Up => [x+1, y]
        };
        let img_val0 = image.get_pixel(x,y)[0];
        let img_val1 = image.get_pixel(comparison_point[0], comparison_point[1])[0];

        // println!("{} {}", img_val0, img_val1);
        // println!("{}", threshold);
        let outside_val = img_val0 as f32 - threshold as f32;
        let inside_val = img_val1 as f32 - threshold as f32;

        let ratio = outside_val / (outside_val - inside_val);
        
        let new_point = [x as f32*ratio + comparison_point[0] as f32 * (1.0-ratio), y as f32*ratio + comparison_point[1] as f32 * (1.0-ratio)];

        contour_points.push(new_point);
    
        match current_direction {
            LookDirection::Right => {
                if image.get_pixel(x+1, y + 1)[0] <= threshold {
                    current_direction = LookDirection::Down;
                    current_point = [x+1, y+1];
                    continue;
                }
                if image.get_pixel(x + 1, y)[0] <= threshold {
                    current_point = [x + 1, y];
                    continue;
                }

                current_direction = LookDirection::Up;
                continue;
            },
            LookDirection::Down => {
                if image.get_pixel(x-1, y + 1)[0] <= threshold {
                    current_direction = LookDirection::Left;
                    current_point = [x-1, y+1];
                    continue;
                }
                if image.get_pixel(x, y+1)[0] <= threshold {
                    current_point = [x, y+1];
                    continue;
                }

                current_direction = LookDirection::Right;
                continue;
            },
            LookDirection::Left => {
                if image.get_pixel(x-1, y - 1)[0] <= threshold {
                    current_direction = LookDirection::Up;
                    current_point = [x-1, y-1];
                    continue;
                }
                if image.get_pixel(x - 1, y)[0] <= threshold {
                    current_point = [x - 1, y];
                    continue;
                }

                current_direction = LookDirection::Down;
                continue;
            },
            LookDirection::Up => {
                if image.get_pixel(x+1, y - 1)[0] <= threshold {
                    current_direction = LookDirection::Right;
                    current_point = [x+1, y-1];
                    continue;
                }
                if image.get_pixel(x, y - 1)[0] <= threshold {
                    current_point = [x, y - 1];
                    continue;
                }

                current_direction = LookDirection::Left;
                continue;
            }
        }
    }



    contour_points
}

#[allow(dead_code)]
fn sdf_image(
    width: u32, 
    height: u32, 
    imgbuf: &ImageBuffer<Luma<u8>, Vec<u8>>) 
    -> ImageBuffer<Luma<u8>, Vec<u8>> {
    
    let mut bitmap: Image2d<Unorm8, Vec<Unorm8>> = sdfer::Image2d::from_fn(width as usize, height as usize, |x, y| {
        let pixel = imgbuf.get_pixel(x as u32, y as u32).channels()[0];
        sdfer::Unorm8::from_bits(pixel)
    });

    let sdf = sdfer::esdt::glyph_to_sdf(&mut bitmap, sdfer::esdt::Params{
        radius: 32.0,
        cutoff: 0.0,
        ..Default::default()
    }, None).0;

    let mut imgbuf2 = image::GrayImage::new(width, height);

    for (x, y, pixel) in imgbuf2.enumerate_pixels_mut() {
        let (xs, ys) = (x as usize, y as usize);
        let sdf_value = sdf[(xs, ys)].to_bits();
        *pixel = Luma([sdf_value]);
    }
    
    imgbuf2
    // imgbuf2.save("C:\\tmp\\sdf.png")?;
    // Ok(())
}

fn remove_points_with_angle(
    scaled_points: Vec<[f32; 2]>, 
    n_points: usize, 
    comparison_angle: f32) -> Vec<[f32; 2]> {
    let mut positions_to_delete: Vec<usize> = vec![];
    let mut current_prev_point = scaled_points[n_points - 1];
    for i in 0..n_points {
        let current_point = scaled_points[i];
        let next_point = scaled_points[(i + 1) % n_points];

        let v0 = normalize(sub(next_point,current_point)); 
        let v1 = normalize(sub(current_prev_point, current_point));
        let angle = (v0[0] * v1[0] + v0[1] * v1[1]).acos();

        if (angle-PI).abs() < comparison_angle {
            positions_to_delete.push(i);
            continue;
        } 

    

        current_prev_point = current_point;
    }

    println!("Points to delete: {}", positions_to_delete.len());
    println!("Points before deletion: {}", n_points);
    let scaled_points = scaled_points.into_iter().enumerate().filter(|(i, _)| !positions_to_delete.contains(i)).map(|(_, p)| p).collect::<Vec<[f32; 2]>>();
    scaled_points
}

fn triangle_from_indices(v0:usize, v1:usize, v2:usize, pos_adjust: usize) -> Primitive {
    Primitive::Triangle(
        (v0 + pos_adjust, Some(v0+pos_adjust), None), 
        (v1 + pos_adjust, Some(v1+pos_adjust), None), 
        (v2 + pos_adjust, Some(v2+pos_adjust), None))
}

fn save_mesh_to_file(
    vertices: impl Iterator<Item = Vertex>, 
    triangles: impl Iterator<Item = Primitive>, 
    uv_vertices: impl Iterator<Item = TVertex>,
    file_path: &str) 
    -> Result<(), Box<dyn Error>> {
   
    let shapes = triangles.map(|triangle| {
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
        vertices: vertices.collect(), 
        tex_vertices: uv_vertices.collect(), 
        normals: vec![], 
        geometry: vec![geometry]
    };

    let obj_set = ObjSet {
        material_library: None,
        objects: vec![mesh]
    };

    obj_exporter::export_to_file(&obj_set, file_path)?;

    Ok(())
}