use image::{RgbImage, Rgb, ImageReader};
use rayon::prelude::*;
use quantette::{ImagePipeline, ColorSpace, QuantizeMethod};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about = "Make any wallpaper fit any colorscheme", long_about = None, max_term_width=120)]
struct Args {
    /// File to generate image from
    input: String,

    /// File to generate image to
    output: String,

    /// Image Palette
    #[arg(long, short, num_args = 0..)]
    palette: Vec<String>,

    /// Blur the image
    #[arg(long, short)]
    blur: bool,

    /// Do not quantize the image before processing (may make the image look better)
    #[arg(long)]
    no_quantize: bool,

    /// Do not dither the image while quantizing
    #[arg(long)]
    no_dither: bool,


    /// Use average algorithm (calculate the average color of each pixel with the pixels around)
    /// to generate the wallpaper, and set the size of the box to calculate the color from.
    /// A value of 0 disables this
    #[arg(long, default_value_t = 0)]
    average: i32,

}

fn color_difference(color1: Rgb::<u8>, color2: Rgb::<u8>) -> u32 {
    return color1.0.iter().zip(color2.0.iter())
                          .map(|(c1, c2)| if c1 > c2 {u32::from(c1-c2)} else {u32::from(c2-c1)})
                          .sum();
}

fn average_color(pixels : Vec<Rgb::<u8>>) -> Rgb<u8> {

    let avg = pixels.iter()
        .map(|pixel| pixel.0) // at this point we have an array of rgb values
        .fold([0,0,0], |mut acc, pixels| {
            for x in 0..=2 {
                acc[x] += usize::from(pixels[x]);  // now we calculate the sum for r,g,b
            };
            return acc
        }
    );

    let red = (avg[0]/pixels.len()).clamp(0,255) as u8; // now we calculate the average
    let green = (avg[1]/pixels.len()).clamp(0,255) as u8;
    let blue = (avg[2]/pixels.len()).clamp(0,255) as u8;
    return Rgb::from([red,green,blue]);
}

// fn average_color(pixels : Vec<Rgb::<u8>>) -> Rgb<u8> {

//     let flat: Vec<[u8; 3]> = pixels.iter()
//         .map(|pixel| pixel.0) // at this point we have an array of rgb values
//         .collect();

//     let r_avg = flat.iter().fold(0, |acc, pixel| acc + usize::from(pixel[0])) / flat.len();
//     let g_avg = flat.iter().fold(0, |acc, pixel| acc + usize::from(pixel[1])) / flat.len();
//     let b_avg = flat.iter().fold(0, |acc, pixel| acc + usize::from(pixel[2])) / flat.len();

//     return Rgb::from([r_avg.clamp(0, 255) as u8, g_avg.clamp(0, 255) as u8 , b_avg.clamp(0, 255) as u8 ]);
// }



fn main() {
    let args = Args::parse();

    // Note: this always outputs rgb8 images due to the into_rgb8 function
    let mut input_img = ImageReader::open(args.input).unwrap().decode().unwrap().into_rgb8();
    let mut output_img = RgbImage::new(input_img.dimensions().0, input_img.dimensions().1);

    let palette = vec![
        Rgb([0, 0, 0 ]),
        Rgb([29, 43, 83 ]),
        Rgb([126, 37, 83 ]),
        Rgb([0, 135, 81 ]),
        Rgb([171, 82, 54 ]),
        Rgb([95, 87, 79 ]),
        Rgb([194, 195, 199 ]),
        Rgb([255, 241, 232 ]),
        Rgb([255, 0, 77 ]),
        Rgb([255, 163, 0 ]),
        Rgb([255, 236, 39 ]),
        Rgb([0, 228, 54 ]),
        Rgb([41, 173, 255 ]),
        Rgb([131, 118, 156 ]),
        Rgb([255, 119, 168 ]),
        Rgb([255, 204, 170 ]),
    ];

    if !args.no_quantize {
        input_img = ImagePipeline::try_from(&input_img).unwrap()
                                                       .palette_size(palette.len() as u8) // set the max number of colors in the palette
                                                       .dither(!args.no_dither) // turn dithering off
                                                       .colorspace(ColorSpace::Oklab) // use a more accurate color space
                                                       .quantize_method(QuantizeMethod::kmeans()) // use a more accurate quantization algorithm
                                                       .quantized_rgbimage_par(); // run the pipeline in parallel to get a [`RgbImage`]
    }

    let output : Vec<Rgb<u8>> = input_img.par_enumerate_pixels().map(|(x,y,pixel)| {
        if args.average > 0 {
            let mut pixel_vec = Vec::<Rgb::<u8>>::new();
            for row in -args.average..args.average {
                for column in -args.average..args.average {
                    if let Some(pixel) = input_img.get_pixel_checked(
                        ((x as i32)+column).clamp(0, input_img.width() as i32).try_into().unwrap(),
                        ((y as i32)+row).clamp(0, input_img.height() as i32).try_into().unwrap()){
                        pixel_vec.push(*pixel);
                    }
                }
            }
            return average_color(pixel_vec)
        } else {
            return *pixel
        }
    }).map(|averaged_pixel|
           palette.iter()
           .map(|color| (color.clone(), color_difference(averaged_pixel, *color)))
           .fold((Rgb::from([0,0,0]),500000), |lowest_current, x| if x.1 < lowest_current.1 {x} else {lowest_current} ).0
    ).collect();

    for i in 0..output.len() as u32{
        let x = i%input_img.width();
        let y = i/input_img.width();
        output_img.put_pixel(x,y,output[i as usize])
    }

    if args.blur{
        output_img = image::imageops::blur(&output_img, 1.0);
    }

    output_img.save(args.output).unwrap();

}
