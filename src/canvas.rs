use std::rc::Rc;
use std::sync::Arc;
use usvg::NodeExt;

pub struct Canvas {
    rtree: usvg::Tree,
    fill: Option<usvg::Fill>,
    stroke: Option<usvg::Stroke>,
}
impl Canvas {
    pub fn new(width: f64, height: f64) -> Self {
        let canvas_size = usvg::Size::new(width, height).unwrap();
        Canvas{
            rtree: usvg::Tree::create(usvg::Svg {
                size: canvas_size,
                view_box: usvg::ViewBox {
                    rect: canvas_size.to_rect(0.0, 0.0),
                    aspect: usvg::AspectRatio::default(),
                },
            }),
            fill: None,
            stroke: None,
        }
    }
    //pub fn set_no_fill(&mut self) { self.fill = None; }
    pub fn set_image_fill(&mut self, id: String) {
        self.fill = Some(usvg::Fill { paint: usvg::Paint::Link(id), ..usvg::Fill::default() });
    }
    pub fn set_color_fill(&mut self, red: u8, green: u8, blue: u8) {
        self.fill = Some(usvg::Fill {
            paint: usvg::Paint::Color(usvg::Color::new_rgb(red, green, blue)),
            ..usvg::Fill::default()
        });
    }
    pub fn set_no_stroke(&mut self) { self.stroke = None; }
    pub fn set_color_stroke(&mut self, red: u8, green: u8, blue: u8, width: f64) {
        self.stroke = Some(usvg::Stroke {
            paint: usvg::Paint::Color(usvg::Color::new_rgb(red, green, blue)),
            width: usvg::StrokeWidth::new(width),
            ..usvg::Stroke::default()
        });
    }
    pub fn add_shape(&mut self, points: &Vec<(f64, f64)>, position: (f64, f64), rotation: f64) {
        let mut path = usvg::PathData::new();
        for (i, point) in points.iter().enumerate() {
            if i == 0 { path.push_move_to(point.0, point.1); }
            else      { path.push_line_to(point.0, point.1); }
        }
        path.push_close_path();
        let mut transform = usvg::Transform::default();
        transform.translate(position.0, position.1);
        transform.rotate(rotation);
        self.rtree.root().append_kind(usvg::NodeKind::Path(usvg::Path {
            fill: self.fill.clone(),
            stroke: self.stroke.clone(),
            data: Rc::new(path),
            transform,
            .. usvg::Path::default()
        }));
    }
    pub fn add_image(&mut self, id: String, data: &Vec<u8>) {
        let mut pattern = self.rtree
            .append_to_defs(usvg::NodeKind::Pattern(usvg::Pattern {
                id,
                units: usvg::Units::ObjectBoundingBox,
                content_units: usvg::Units::UserSpaceOnUse,
                transform: usvg::Transform::default(),
                rect: usvg::Rect::new(0.0, 0.0, 1.0, 1.0).unwrap(),
                view_box: Some(usvg::ViewBox {
                    rect: usvg::Rect::new(0.0, 0.0, 1.0, 1.0).unwrap(),
                    aspect: usvg::AspectRatio{
                        defer: false,
                        align: usvg::Align::XMidYMid,
                        slice: true,
                    },
                }),
            }));
        pattern.append_kind(usvg::NodeKind::Path(usvg::Path {
            fill: Some(usvg::Fill {
                paint: usvg::Paint::Color(usvg::Color::new_rgb(255, 255, 255)),
                ..usvg::Fill::default()
            }),
            data: Rc::new(usvg::PathData::from_rect(usvg::Rect::new(0.0, 0.0, 1.0, 1.0).unwrap())),
            .. usvg::Path::default()
        }));
        enum ImageFormat { PNG, JPEG, GIF }
        let data_type = if data.starts_with(b"\x89PNG\r\n\x1a\n") {
            Some(ImageFormat::PNG)
        } else if data.starts_with(&[0xff, 0xd8, 0xff]) {
            Some(ImageFormat::JPEG)
        } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
            Some(ImageFormat::GIF)
        } else {
            None
        };
        let kind_node = match data_type {
            Some(ImageFormat::JPEG) => Some(usvg::ImageKind::JPEG(Arc::new(data.clone()))),
            Some(ImageFormat::PNG)  => Some(usvg::ImageKind::PNG (Arc::new(data.clone()))),
            Some(ImageFormat::GIF)  => Some(usvg::ImageKind::GIF (Arc::new(data.clone()))),
            _ => None,
        };
        if let Some(kind_node) = kind_node {
            pattern.append_kind(usvg::NodeKind::Image(usvg::Image{
                id: "".into(),
                transform: usvg::Transform::default(),
                visibility: usvg::Visibility::Visible,
                view_box: usvg::ViewBox {
                    rect: usvg::Rect::new(0.0, 0.0, 1.0, 1.0).unwrap(),
                    aspect: usvg::AspectRatio::default(),
                },
                rendering_mode: usvg::ImageRendering::OptimizeQuality,
                kind: kind_node,
            }));
        }
    }
    //pub fn encode_svg(&self) -> String {
    //    return self.rtree.to_string(&usvg::XmlOptions::default());
    //}
    pub fn encode_png(&self) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync + 'static>> {
        let pixmap_size = self.rtree.svg_node().size.to_screen_size();
        let mut pixmap = tiny_skia::Pixmap::new(pixmap_size.width(), pixmap_size.height()).unwrap();
        resvg::render(&self.rtree, usvg::FitTo::Original, tiny_skia::Transform::default(), pixmap.as_mut()).unwrap();
        Ok(pixmap.encode_png()?)
    }
    //pub fn save_png(&self, path: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    //    let data = self.encode_png()?;
    //    std::fs::write(path, data)?;
    //    Ok(())
    //}
    pub fn load_shaper_from_svg(path: &str, scale: f64) -> Result<Vec<Vec<(f64, f64)>>, Box<dyn std::error::Error + Send + Sync + 'static>> {
        let svg_data = std::fs::read(path)?;
        let opt = usvg::Options::default();
        let rtree = usvg::Tree::from_data(&svg_data, &opt.to_ref())?;
        let mut shapes: Vec<Vec<(f64, f64)>> = Vec::new();
        for node in rtree.root().descendants() {
            if !rtree.is_in_defs(&node) {
                let node = (*node.borrow()).clone();
                if let usvg::NodeKind::Path(path) = node {
                    let mut shape: Vec<(f64, f64)> = Vec::new();
                    let mut rect = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
                    for point in path.data.iter() {
                        let point: Option<(f64, f64)> = match point {
                            usvg::PathSegment::MoveTo{ x, y } => Some((*x, *y)),
                            usvg::PathSegment::LineTo{ x, y } => Some((*x, *y)),
                            _ => None
                        };
                        if let Some(point) = point {
                            if point.0 < rect.0 { rect.0 = point.0; }
                            if point.1 < rect.1 { rect.1 = point.1; }
                            if rect.2 < point.0 { rect.2 = point.0; }
                            if rect.3 < point.1 { rect.3 = point.1; }
                            shape.push(point);
                        }
                    }
                    let center = ((rect.0 + rect.2) * 0.5, (rect.1 + rect.3) * 0.5);
                    shape.iter_mut().for_each(|point| {
                        point.0 = (point.0 - center.0) * scale;
                        point.1 = (point.1 - center.1) * scale;
                    });
                    shapes.push(shape.clone());
                    shapes.push(shape.iter().map(|(x, y)| (-x, *y)).collect());
                }
            }
        }
        Ok(shapes)
    }
}
