#[macro_use]
extern crate glium;
extern crate clap;
extern crate glutin;
extern crate rayon;
extern crate lyon;
extern crate lyon_svg;

use glium::Surface;
use glium::index::PrimitiveType;
use glium::DisplayBuild;
use glium::backend::glutin_backend::GlutinFacade as Display;

use lyon::math::*;
use lyon::path::default::Path;
use lyon::path::builder::*;
use lyon::tessellation::geometry_builder::{ VertexConstructor, VertexBuffers, BuffersBuilder };
use lyon::tessellation::{ FillEvents, FillTessellator, FillOptions, FillVertex };
use lyon::tessellation::{ StrokeTessellator, StrokeOptions };
use lyon::tessellation::StrokeVertex;
use lyon::geom::euclid::{Transform3D, vec3};
use lyon_svg::parser as parser;
use parser::Color;
use parser::FromSpan;

use clap::Arg;

fn main() {
    let matches = clap::App::new("SVG renderer test")
        .version("0.1")
        .about("Renders a SVG passed as parameter")
        .arg(Arg::with_name("INPUT")
             .help("Sets the input SVG file")
             .takes_value(true)
             .index(1)
             .required(true))
        .arg(Arg::with_name("OUTPUT")
             .help("Sets the output file to export the tessellated geometry (optional).")
             .takes_value(true)
             .short("o")
             .long("output")
             .required(false))
        .get_matches();

    if let Some(output) = matches.value_of("OUTPUT") {
        println!("output {:?}", output);
    } else {
        println!("Rendering to a window");
    }

    let mut scene = if let Some(input_file) = matches.value_of("INPUT") {
        load_svg(input_file)
    } else {
        unimplemented!();
    };

    tessellate_scene(&mut scene[..]);

    let display = glutin::WindowBuilder::new()
        .with_dimensions(700, 700)
        .with_title("tessellation".to_string())
        .with_multisampling(8)
        .with_vsync()
        .build_glium().unwrap();

    upload_geometry(&mut scene[..], &display);

/*
    let model_vbo = glium::VertexBuffer::new(&display, &vertices[..]).unwrap();
    let model_ibo = glium::IndexBuffer::new(
        &display, PrimitiveType::TrianglesList,
        &indices[..]
    ).unwrap();

    let bg_vbo = glium::VertexBuffer::new(&display, &bg_buffers.vertices[..]).unwrap();
    let bg_ibo = glium::IndexBuffer::new(
        &display, PrimitiveType::TrianglesList,
        &bg_buffers.indices[..]
    ).unwrap();
    // compiling shaders and linking them together
    let bg_program = program!(&display,
        140 => {
            vertex: "
                #version 140
                in vector a_position;
                out vector v_position;
                void main() {
                    gl_Position = vec4(a_position, 0.0, 1.0);
                    v_position = a_position;
                }
            ",
            fragment: "
                #version 140
                uniform vector u_resolution;
                in vector v_position;
                out vec4 f_color;
                void main() {
                    vector px_position = (v_position * vector(1.0, -1.0)    + vector(1.0, 1.0))
                                     * 0.5 * u_resolution;
                    // #005fa4
                    float vignette = clamp(0.0, 1.0, (0.7*length(v_position)));

                    f_color = mix(
                        vec4(0.0, 0.47, 0.9, 1.0),
                        vec4(0.0, 0.1, 0.64, 1.0),
                        vignette
                    );

                    if (mod(px_position.x, 20.0) <= 1.0 ||
                        mod(px_position.y, 20.0) <= 1.0) {
                        f_color *= 1.2;
                    }

                    if (mod(px_position.x, 100.0) <= 1.0 ||
                        mod(px_position.y, 100.0) <= 1.0) {
                        f_color *= 1.2;
                    }
                }
            "
        },
    ).unwrap();
*/

    // compiling shaders and linking them together
    let model_program = program!(&display,
        140 => {
            vertex: "
                #version 140
                uniform vector u_resolution;
                uniform mat4 u_matrix;
                in vector a_position;
                in vec3 a_color;
                out vec3 v_color;
                void main() {
                    gl_Position = u_matrix * vec4(a_position, 0.0, 1.0);// / vec4(u_resolution, 1.0, 1.0);
                    v_color = a_color;
                }
            ",
            fragment: "
                #version 140
                in vec3 v_color;
                out vec4 f_color;
                void main() {
                    f_color = vec4(v_color, 1.0);
                }
            "
        },
    ).unwrap();

    let mut target_zoom = 1.0;
    let mut zoom = 1.0;
    let mut target_pos: Point = point(0.0, 0.0);
    let mut pos = point(0.0, 0.0);
    loop {
        zoom += (target_zoom - zoom) / 3.0;
        pos = pos + (target_pos - pos) / 3.0;

        let mut target = display.draw();

        let (w, h) = target.get_dimensions();
        let resolution: Vector = vector(w as f32, h as f32);

        let model_mat = Transform3D::identity();
        let view_mat = Transform3D::identity()
            .pre_translate(vec3(-1.0, 1.0, 0.0))
            .pre_scale(5.0 * zoom, 5.0 * zoom, 0.0)
            .pre_scale(2.0/resolution.x, -2.0/resolution.y, 1.0)
            .pre_translate(vec3(pos.x, pos.y, 0.0));

        let uniforms = uniform! {
            u_resolution: resolution.to_array(),
            u_matrix: uniform_matrix(&model_mat.pre_mul(&view_mat))
        };

        target.clear_color(0.75, 0.75, 0.75, 1.0);

        for item in &scene[..] {
            if let &Some((ref vbo, ref ibo)) = &item.uploaded {
                target.draw(
                    vbo, ibo,
                    &model_program, &uniforms,
                    &Default::default()
                ).unwrap();
            }
        }

        target.finish().unwrap();

        let mut should_close = false;
        for event in display.poll_events() {
            should_close |= match event {
                glutin::Event::Closed => true,
                glutin::Event::KeyboardInput(_, _, Some(glutin::VirtualKeyCode::Escape)) => true,
                glutin::Event::KeyboardInput(_, _, Some(glutin::VirtualKeyCode::PageDown)) => {
                    target_zoom *= 0.8;
                    false
                }
                glutin::Event::KeyboardInput(_, _, Some(glutin::VirtualKeyCode::PageUp)) => {
                    target_zoom *= 1.25;
                    false
                }
                glutin::Event::KeyboardInput(_, _, Some(glutin::VirtualKeyCode::Left)) => {
                    target_pos.x += 5.0 / target_zoom;
                    false
                }
                glutin::Event::KeyboardInput(_, _, Some(glutin::VirtualKeyCode::Right)) => {
                    target_pos.x -= 5.0 / target_zoom;
                    false
                }
                glutin::Event::KeyboardInput(_, _, Some(glutin::VirtualKeyCode::Up)) => {
                    target_pos.y += 5.0 / target_zoom;
                    false
                }
                glutin::Event::KeyboardInput(_, _, Some(glutin::VirtualKeyCode::Down)) => {
                    target_pos.y -= 5.0 / target_zoom;
                    false
                }
                _evt => {
                    //println!("{:?}", _evt);
                    false
                }
            };
        }
        if should_close {
            break;
        }
    }
}

use std::fs;
use std::io::Read;
use parser::svg::Token as SvgToken;

struct RenderItem {
    path: Path,
    fill: Option<Color>,
    stroke: Option<Color>,
    stroke_width: f32,
    geometry: Option<VertexBuffers<Vertex>>,
    uploaded: Option<(glium::VertexBuffer<Vertex>, glium::IndexBuffer<u16>)>,
}

impl RenderItem {
    fn new() -> RenderItem {
        RenderItem {
            path: Path::new(),
            fill: None,
            stroke: None,
            stroke_width: 1.0,
            geometry: None,
            uploaded: None,
        }

    }
}

fn tessellate_scene(scene: &mut[RenderItem]) {
    println!(" -- The scene contains {} items to tessellate", scene.len());
    let mut fill_tessellator = FillTessellator::new();
    let mut fill_events = FillEvents::new();
    let mut stroke_tessellator = StrokeTessellator::new();

    for item in scene {
        if item.geometry.is_none() {
            let mut buffers: VertexBuffers<Vertex> = VertexBuffers::new();
            if let Some(color) = item.fill {
                println!(" -- tessellate fill");
                fill_events.set_path(0.03, item.path.path_iter());
                fill_tessellator.tessellate_events(
                    &fill_events,
                    &FillOptions::default(),
                    &mut BuffersBuilder::new(&mut buffers, WithColor(
                        [color.red as f32 / 255.0, color.green as f32 / 255.0, color.blue as f32 / 255.0]
                    ))
                ).unwrap();
            }
            if let Some(color) = item.stroke {
                println!(" -- tessellate stroke");
                stroke_tessellator.tessellate_path(
                    item.path.path_iter(),
                    &StrokeOptions::tolerance(0.03),
                    &mut BuffersBuilder::new(&mut buffers, WithColorAndStrokeWidth([
                            color.red as f32 / 255.0,
                            color.green as f32 / 255.0,
                            color.blue as f32 / 255.0
                        ],
                        item.stroke_width
                    ))
                );
                //item.geometry = Some(buffers);
                //item.uploaded = None;
            }
            item.geometry = Some(buffers);
            item.uploaded = None;
        }
    }
}

fn upload_geometry(scene: &mut[RenderItem], display: &Display) {
    for item in scene {
        let uploaded = match (&item.geometry, &item.uploaded) {
            (&Some(ref geom), &None) => {
                let vbo = glium::VertexBuffer::new(display, &geom.vertices[..]).unwrap();
                let ibo = glium::IndexBuffer::new(
                    display, PrimitiveType::TrianglesList,
                    &geom.indices[..]
                ).unwrap();

                Some((vbo, ibo))
            }
            _ => { None }
        };

        if uploaded.is_some() {
            println!(" -- upload geometry");
            item.uploaded = uploaded;
        }
    }
}

fn load_svg(file_name: &str) -> Vec<RenderItem> {
    println!("-- loading {:?}", file_name);

    // Read a file to the buffer.
    let mut file = fs::File::open(file_name).unwrap();
    let mut buffer = String::new();
    file.read_to_string(&mut buffer).unwrap();

    let mut render_items = Vec::new();
    let mut current_item = RenderItem::new();

    for item in parser::svg::Tokenizer::from_str(&buffer) {
        match item {
            Ok(SvgToken::ElementStart(parser::svg::Name::Svg(parser::ElementId::Path))) => {
                current_item = RenderItem::new();
            }
            Ok(SvgToken::ElementEnd(_)) => {
                println!(" -- close path");
                if current_item.fill.is_some() || current_item.stroke.is_some() {
                    let mut tmp = RenderItem::new();
                    ::std::mem::swap(&mut current_item, &mut tmp);
                    render_items.push(tmp);
                }
            }
            Ok(SvgToken::Attribute(parser::svg::Name::Svg(parser::AttributeId::Style), span)) => {
                parse_style(span, &mut current_item);
            }
            Ok(SvgToken::Attribute(parser::svg::Name::Svg(parser::AttributeId::Path), span)) => {
                current_item.path = parse_path_data(span);
            }
            _ => {}
        }
    }

    println!(" -- loaded {} paths", render_items.len());

    return render_items;
}

fn parse_path_data(span: parser::StrSpan) -> Path {
    let mut builder = Path::builder().with_svg();

    for item in lyon_svg::path_utils::PathTokenizer::from_span(span) {
        match item {
            Ok(evt) => { builder.svg_event(evt) }
            Err(e) => { panic!("Warning: {:?}.", e); }
        }
    }

    return builder.build();
}

fn parse_style(span: parser::StrSpan, item: &mut RenderItem) {
    use parser::{AttributeId, AttributeValue, ValueId, ElementId};

    for attr in parser::style::Tokenizer::from_span(span) {
        if let Ok(parser::style::Token::SvgAttribute(id, span)) = attr {
            let value = AttributeValue::from_span(
                ElementId::Path,
                AttributeId::Style,
                span
            ).unwrap_or(AttributeValue::PredefValue(ValueId::None));

            match id {
                AttributeId::Fill => {
                    match value {
                        AttributeValue::Color(rgb) => { item.fill = Some(rgb); }
                        AttributeValue::PredefValue(ValueId::None) => { item.fill = None; }
                        _ => { item.fill = Some(Color { red: 255, green: 0, blue: 0 }) }
                    }
                }
                AttributeId::Stroke => {
                    match value {
                        AttributeValue::Color(rgb) => { item.stroke = Some(rgb); }
                        AttributeValue::PredefValue(ValueId::None) => { item.stroke = None; }
                        _ => { item.stroke = Some(Color { red: 255, green: 0, blue: 0 }) }
                    }
                }
                AttributeId::StrokeWidth => {
                    match value {
                        AttributeValue::Number(n) => {
                            item.stroke_width = n as f32;
                        }
                        AttributeValue::Length(lyon_svg::parser::Length { num, .. }) => {
                            item.stroke_width = num as f32;
                        }
                        _=> {
                            panic!(" stroke-width: {:?}", value);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

#[derive(Copy, Clone, Debug)]
struct Vertex {
    a_position: [f32; 2],
    a_color: [f32; 3],
}

struct WithColor([f32; 3]);

impl VertexConstructor<FillVertex, Vertex> for WithColor {
    fn new_vertex(&mut self, v: FillVertex) -> Vertex {
        assert!(!v.position.x.is_nan());
        assert!(!v.position.y.is_nan());
        Vertex {
            a_position: v.position.to_array(),
            a_color: self.0,
        }
    }
}

struct WithColorAndStrokeWidth([f32; 3], f32);

impl VertexConstructor<StrokeVertex, Vertex> for WithColorAndStrokeWidth {
    fn new_vertex(&mut self, vertex: StrokeVertex) -> Vertex {
        assert!(!vertex.position.x.is_nan());
        assert!(!vertex.position.y.is_nan());
        assert!(!vertex.normal.x.is_nan());
        assert!(!vertex.normal.y.is_nan());
        Vertex {
            a_position: (vertex.position + vertex.normal * self.1).to_array(),
            a_color: self.0,
        }
    }
}

implement_vertex!(Vertex, a_position, a_color);
/*
#[derive(Copy, Clone, Debug)]
struct BgVertex {
    a_position: [f32; 2],
}

struct BgWithColor ;
impl VertexConstructor<Vector, BgVertex> for BgWithColor  {
    fn new_vertex(&mut self, pos: Vector) -> BgVertex {
        BgVertex { a_position: pos.to_array() }
    }
}
implement_vertex!(BgVertex, a_position);
*/

fn uniform_matrix(m: &Transform3D<f32>) -> [[f32; 4]; 4] {
    [
        [m.m11, m.m12, m.m13, m.m14],
        [m.m21, m.m22, m.m23, m.m24],
        [m.m31, m.m32, m.m33, m.m34],
        [m.m41, m.m42, m.m43, m.m44],
    ]
}
