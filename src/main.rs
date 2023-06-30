use png_decode::*;
fn main() {
    let photos = fs::read_dir("./test_images").unwrap();

    for photo in photos {

        let name = photo.unwrap().path();
        let name = name.to_str().unwrap();

        match &name[name.len()-4..] {
            ".jpg" => println!("Not finished, runned into a lot of specification details that would not teach me anythin new from PNG, but waste a lot of time"),
            ".png" => {
                let img= PNGImage::from_path(name);
                //println!("{:?}", img.header);
                let hd = img.header;
                let img = ImgData::new(hd.width, hd.height, &img.data, hd.color_type, hd.bit_depth);
                let (width, _height) = crossterm::terminal::size().expect("No acces to shell");
                let img = resize(img, width as f32);
                print(img);
            }
            n => println!("Extension {} not supported", n),
        }
    }
}
