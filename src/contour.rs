use std::{f32::consts::PI, ops::Index};
use image::{DynamicImage, GenericImageView, GrayImage, ImageBuffer, Luma, Pixel};
use sdfer::{Image2d, Unorm8};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LookDirection {
    Right,
    Down,
    Left,
    Up
}

#[derive(Debug, Clone)]
pub struct Contour(Vec<[f32; 2]>);

impl Index<usize> for Contour {
    type Output = [f32; 2];

    fn index(&self, index: usize) -> &[f32; 2] {
        &self.0[index]
    }
}

impl FromIterator<[f32; 2]> for Contour {
    fn from_iter<I: IntoIterator<Item = [f32; 2]>>(iter: I) -> Self {
        Contour(iter.into_iter().collect())
    }
}

impl Contour {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn push(&mut self, point: [f32; 2]) {
        self.0.push(point);
    }

    pub fn iter(&self) -> std::slice::Iter<'_, [f32; 2]> {
        self.0.iter()
    }

    pub fn new() -> Contour {
        Contour(vec![])
    }

    pub fn into_iter(self) -> std::vec::IntoIter<[f32; 2]> {
        self.0.into_iter()
    }

    pub fn smooth(self, iterations: u32) -> Contour {
        let mut smoothed_contour:Contour = self.clone();
    
        for _ in 0..iterations {
            // Could technically swap between self and smoothed_contour to avoid creating a new vector every iteration.
            // Won't bother to do it though.
            let mut curr_smoothed_contour:Contour = Contour::new();
    
            for i in 0..smoothed_contour.len() {
                let prev = smoothed_contour[(i + smoothed_contour.len() - 1) % smoothed_contour.len()];
                let current = smoothed_contour[i];
                let next = smoothed_contour[(i + 1) % smoothed_contour.len()];
        
                let smoothed = [(prev[0] + current[0] + next[0]) / 3.0, (prev[1] + current[1] + next[1]) / 3.0];
                curr_smoothed_contour.push(smoothed);
            }
            smoothed_contour = curr_smoothed_contour;
        }
    
        smoothed_contour
    }
    
    pub fn simplify(
        self, 
        comparison_angle: f32)
        -> Contour {
    
        let n_points = self.len();
        let mut should_be_deleted: Vec<bool> = vec![false; n_points];
        let mut current_prev_point = self[n_points - 1];
        for i in 0..n_points {
            let current_point = self[i];
            let next_point = self[(i + 1) % n_points];
    
            let v0 = normalize(sub(next_point,current_point)); 
            let v1 = normalize(sub(current_prev_point, current_point));
            let angle = (v0[0] * v1[0] + v0[1] * v1[1]).acos();
    
            if (angle-PI).abs() < comparison_angle {
                should_be_deleted[i] = true;
                continue;
            } 
    
            current_prev_point = current_point;
        }
    
        self.into_iter()
        .enumerate()
        .filter(|(i, _)| should_be_deleted[*i])
        .map(|(_, p)| p)
        .collect()
    }
    
    pub fn scale(self, width: f32, height: f32) -> Contour {
        self.into_iter().map(|p| [p[0] / width, p[1] / height]).collect()
    }

}

pub struct Params {
    pub border_offset: f32,
    pub smooth_iterations: u32,
    pub simplify_angle: f32,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            border_offset: 20.0,
            smooth_iterations: 10,
            simplify_angle: PI/30.0,
        }
    }
}

pub fn find_contour_from_transparency_with_offset(img: &DynamicImage, params: Params) -> Result<Contour, &'static str> {

    let (width, height) = img.dimensions();

    let mut imgbuf = image::GrayImage::new(width, height);    

    for (x, y, pixel) in imgbuf.enumerate_pixels_mut() {
        *pixel = Luma([img.get_pixel(x, y).channels()[3]]);
    }

    let sdf = sdf_image(width, height, params.border_offset, &imgbuf);

    let (f_width, f_height) = (width as f32, height as f32);

    Ok(
        find_contour_from_grayscale(&sdf, 128u8)?
        .smooth(params.smooth_iterations)
        .scale(f_width, f_height)
        .simplify(params.simplify_angle))
}

pub fn find_contour_from_grayscale(image: &GrayImage, threshold: u8) -> Result<Contour, &'static str> {
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
        return Err("No starting point found in the grayscale image.");
    }

    let start_point = start_point.unwrap();
    let mut contour:Contour = Contour::new();

    let mut current_direction = LookDirection::Right;
    let mut current_point = start_point;

    let max_iterations = image.width() * image.height();
    let mut sanity_check = 0;

    loop{

        sanity_check += 1;

        if sanity_check > max_iterations {
            return Err("It was not possible to find a contour in the image.");
        }

        // When we come back to the starting point, we're done
        if contour.len() > 0 && current_point==start_point { break;}

        let (x, y) = (current_point[0], current_point[1]);

        let comparison_point = match current_direction {
            LookDirection::Right => [x, y + 1],
            LookDirection::Down => [x-1, y],
            LookDirection::Left => [x, y-1],
            LookDirection::Up => [x+1, y]
        };
        let img_val0 = image.get_pixel(x,y)[0];
        let img_val1 = image.get_pixel(comparison_point[0], comparison_point[1])[0];

        let outside_val = img_val0 as f32 - threshold as f32;
        let inside_val = img_val1 as f32 - threshold as f32;

        let ratio = outside_val / (outside_val - inside_val);
        
        let new_point = [x as f32*ratio + comparison_point[0] as f32 * (1.0-ratio), y as f32*ratio + comparison_point[1] as f32 * (1.0-ratio)];

        contour.push(new_point);
    
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

    Ok(contour)
}


fn sub(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

fn normalize(v: [f32; 2]) -> [f32; 2] {
    let norm = (v[0] * v[0] + v[1] * v[1]).sqrt();
    [v[0] / norm, v[1] / norm]
}

fn sdf_image(
    width: u32, 
    height: u32,
    offset: f32,
    imgbuf: &ImageBuffer<Luma<u8>, Vec<u8>>) 
    -> ImageBuffer<Luma<u8>, Vec<u8>> {
    
    let mut bitmap: Image2d<Unorm8, Vec<Unorm8>> = sdfer::Image2d::from_fn(width as usize, height as usize, |x, y| {
        let pixel = imgbuf.get_pixel(x as u32, y as u32).channels()[0];
        sdfer::Unorm8::from_bits(pixel)
    });

    let sdf = sdfer::esdt::glyph_to_sdf(&mut bitmap, sdfer::esdt::Params{
        radius: offset,
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
}