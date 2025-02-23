//! The material used for terminal rendering.
//!
//! By default the terminal expects a [code page 437](https://dwarffortresswiki.org/Tileset_repository)
//! texture with 16x16 characters. New font textures can be added to the assets directory and loaded via
//! the bevy `AssetLoader`.
//!
//! To change the terminal font, you must assign a new `Handle<Image>` to the material's `texture` field:
//! ```
//! use bevy::prelude::*;
//! use bevy_ascii_terminal::*;
//! fn change_font(
//! asset_server: Res<AssetServer>,
//! mut materials: ResMut<Assets<TerminalMaterial>>,
//! mut q_term: Query<&Handle<TerminalMaterial>>,
//! ) {
//!     for mat in q_term.iter_mut() {
//!         let mut mat = materials.get_mut(mat).unwrap();
//!         let new_font = asset_server.load("some_cool_font.png");
//!         mat.texture = Some(new_font);
//!     }
//! }
//! ```
//!
//! The terminal comes with several built in fonts:
//! - jt_curses_12x12.png
//! - pastiche_8x8.png
//! - px437_8x8.png
//! - taffer_10x10.png
//! - zx_evolution_8x8.png
//!
//! These fonts can be accessed via the [BuiltInFontHandles] resource:
//!
//! The `TerminalMaterial` also has a `clip_color` field. This field is used by the shader
//! to determine what constitutes a "background color" on the terminal texture.

use bevy::app::{App, Plugin};
use bevy::asset::{AssetServer, Assets, Handle, HandleUntyped};
use bevy::ecs::system::{lifetimeless::SRes, SystemParamItem};
use bevy::math::Vec4;
use bevy::prelude::Mesh;
use bevy::reflect::TypeUuid;
use bevy::render::mesh::MeshVertexBufferLayout;
use bevy::render::texture::ImageType;
use bevy::render::{
    color::Color,
    prelude::Shader,
    render_asset::{PrepareAssetError, RenderAsset, RenderAssets},
    render_resource::{
        std140::{AsStd140, Std140},
        *,
    },
    renderer::RenderDevice,
    texture::Image,
};

use bevy::sprite::{Material2dPipeline, Material2dPlugin, SpecializedMaterial2d};
use bevy::utils::HashMap;

use super::plugin::{ATTRIBUTE_COLOR_BG, ATTRIBUTE_COLOR_FG, ATTRIBUTE_UV};

/// The default shader handle used by the terminal.
pub const TERMINAL_MATERIAL_SHADER_HANDLE: HandleUntyped =
    HandleUntyped::weak_from_u64(Shader::TYPE_UUID, 3142086872234592509);

/// The default material handle used by the terminal.
pub const TERMINAL_DEFAULT_MATERIAL_HANDLE: HandleUntyped =
    HandleUntyped::weak_from_u64(Shader::TYPE_UUID, 2121056571224552501);

macro_rules! include_font {
    ($font_name:expr) => {{
        let bytes = include_bytes!(concat!("builtin/", $font_name));
        (
            $font_name,
            Image::from_buffer(
                bytes,
                ImageType::Extension("png"),
                bevy::render::texture::CompressedImageFormats::NONE,
                false,
            )
            .unwrap(),
        )
    }};
}

/// A resource which can be used to retrieve the image handles
/// for the terminal's built-in fonts.
///
/// # Example
///
/// ```
/// use bevy::prelude::*;
/// use bevy_ascii_terminal::*;
/// fn change_font_built_in(
/// fonts: Res<BuiltInFontHandles>,
/// mut materials: ResMut<Assets<TerminalMaterial>>,
/// q_mat: Query<&Handle<TerminalMaterial>>,
/// ) {
///     for mat in q_mat.iter() {
///         let mut mat = materials.get_mut(mat).unwrap();
///         let built_in = fonts.get("zx_evolution_8x8.png").unwrap();
///
///         mat.texture = Some(built_in.clone());
///     }
/// }
/// ```
pub struct BuiltInFontHandles {
    map: HashMap<String, Handle<Image>>,
}

impl BuiltInFontHandles {
    /// Retrieve a built-in font handle by it's name. Must include ".png" the extension.
    pub fn get(&self, font_name: &str) -> Option<&Handle<Image>> {
        self.map.get(font_name)
    }

    /// An iterator over the name-value-pairs of the built in font handles
    /// for the terminal.
    ///
    /// Yields (&String, &Handle<Image>), where `String` is the file name of the
    /// font texture.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Handle<Image>)> {
        self.map.iter()
    }
}

/// Plugin for the terminal renderer. Initializes resources and systems related to rendering.
#[derive(Default)]
pub struct TerminalMaterialPlugin;

impl Plugin for TerminalMaterialPlugin {
    fn build(&self, app: &mut App) {
        let mut shaders = app.world.get_resource_mut::<Assets<Shader>>().unwrap();

        shaders.set_untracked(
            TERMINAL_MATERIAL_SHADER_HANDLE,
            Shader::from_wgsl(include_str!("terminal.wgsl")),
        );
        app.add_plugin(Material2dPlugin::<TerminalMaterial>::default());

        let mut fonts = BuiltInFontHandles {
            map: HashMap::default(),
        };
        let font_map = &mut fonts.map;

        let mut images = app.world.get_resource_mut::<Assets<Image>>().unwrap();

        let font = include_font!("jt_curses_12x12.png");
        add_font_resource(font, &mut images, font_map);

        let font = include_font!("pastiche_8x8.png");
        add_font_resource(font, &mut images, font_map);

        let font = include_font!("px437_8x8.png");
        let default_font = add_font_resource(font, &mut images, font_map);

        let font = include_font!("taffer_10x10.png");
        add_font_resource(font, &mut images, font_map);

        let font = include_font!("zx_evolution_8x8.png");
        add_font_resource(font, &mut images, font_map);

        app.world
            .get_resource_mut::<Assets<TerminalMaterial>>()
            .unwrap()
            .set_untracked(Handle::<TerminalMaterial>::default(), default_font.into());

        app.insert_resource(fonts);
    }
}

fn add_font_resource(
    font: (&str, Image),
    images: &mut Assets<Image>,
    font_map: &mut HashMap<String, Handle<Image>>,
) -> Handle<Image> {
    let handle = images.set(font.0, font.1);
    font_map.insert(font.0.to_string(), handle.clone());
    handle
}

/// The material for rendering a terminal.
#[derive(Debug, Clone, TypeUuid)]
#[uuid = "e228a534-e3ca-2e1e-ab9d-4d8bc1ad8c19"]
pub struct TerminalMaterial {
    /// The clip color for the active font texture.
    ///
    /// Clip color determines which part of the texture is regarded as
    /// "background color".
    pub clip_color: Color,

    /// The font texture rendered by the terminal.
    pub texture: Option<Handle<Image>>,
}

impl Default for TerminalMaterial {
    fn default() -> Self {
        TerminalMaterial {
            clip_color: Color::BLACK,
            texture: None,
        }
    }
}

impl From<Handle<Image>> for TerminalMaterial {
    fn from(texture: Handle<Image>) -> Self {
        TerminalMaterial {
            texture: Some(texture),
            clip_color: Color::BLACK,
        }
    }
}

// NOTE: These must match the bit flags in shader.wgsl!
bitflags::bitflags! {
    #[repr(transparent)]
    pub struct TerminalMaterialFlags: u32 {
        const TEXTURE           = (1 << 0);
        const NONE              = 0;
        const UNINITIALIZED     = 0xFFFF;
    }
}

/// The GPU representation of the uniform data of a [`TerminalMaterial`].
#[derive(Clone, Default, AsStd140)]
struct TerminalMaterialUniformData {
    pub color: Vec4,
    pub flags: u32,
}

// The data from our material that gets copied to the gpu
#[derive(Debug, Clone)]
pub struct GpuTerminalMaterial {
    pub buffer: Buffer,
    pub bind_group: BindGroup,
    pub flags: TerminalMaterialFlags,
    pub texture: Option<Handle<Image>>,
}

// Boilerplate copied from `ColorMaterial`. Allows us to reference
// our texture and mesh/view structs from the shader.
impl RenderAsset for TerminalMaterial {
    type ExtractedAsset = TerminalMaterial;
    type PreparedAsset = GpuTerminalMaterial;
    type Param = (
        SRes<RenderDevice>,
        SRes<Material2dPipeline<TerminalMaterial>>,
        SRes<RenderAssets<Image>>,
    );

    fn extract_asset(&self) -> Self::ExtractedAsset {
        self.clone()
    }

    fn prepare_asset(
        material: Self::ExtractedAsset,
        (render_device, pipeline, gpu_images): &mut SystemParamItem<Self::Param>,
    ) -> Result<Self::PreparedAsset, PrepareAssetError<Self::ExtractedAsset>> {
        let (texture_view, sampler) = if let Some(result) = pipeline
            .mesh2d_pipeline
            .get_image_texture(gpu_images, &material.texture)
        {
            result
        } else {
            return Err(PrepareAssetError::RetryNextUpdate(material));
        };

        let mut flags = TerminalMaterialFlags::NONE;
        if material.texture.is_some() {
            flags |= TerminalMaterialFlags::TEXTURE;
        }

        let value = TerminalMaterialUniformData {
            color: material.clip_color.as_linear_rgba_f32().into(),
            flags: flags.bits(),
        };
        let value_std140 = value.as_std140();

        let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("terminal_material_uniform_buffer"),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            contents: value_std140.as_bytes(),
        });
        let bind_group = render_device.create_bind_group(&BindGroupDescriptor {
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(texture_view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Sampler(sampler),
                },
            ],
            label: Some("terminal_material_bind_group"),
            layout: &pipeline.material2d_layout,
        });

        Ok(GpuTerminalMaterial {
            buffer,
            bind_group,
            flags,
            texture: material.texture,
        })
    }
}

impl SpecializedMaterial2d for TerminalMaterial {
    fn fragment_shader(_asset_server: &AssetServer) -> Option<Handle<Shader>> {
        Some(TERMINAL_MATERIAL_SHADER_HANDLE.typed())
    }

    fn vertex_shader(_asset_server: &AssetServer) -> Option<Handle<Shader>> {
        Some(TERMINAL_MATERIAL_SHADER_HANDLE.typed())
    }

    #[inline]
    fn bind_group(render_asset: &<Self as RenderAsset>::PreparedAsset) -> &BindGroup {
        &render_asset.bind_group
    }

    fn bind_group_layout(render_device: &RenderDevice) -> BindGroupLayout {
        render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: BufferSize::new(
                            TerminalMaterialUniformData::std140_size_static() as u64,
                        ),
                    },
                    count: None,
                },
                // Texture
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        multisampled: false,
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                    },
                    count: None,
                },
                // Texture Sampler
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
            label: Some("color_material_layout"),
        })
    }

    type Key = ();

    fn key(
        _render_devicec: &RenderDevice,
        _material: &<Self as RenderAsset>::PreparedAsset,
    ) -> Self::Key {
    }

    fn specialize(
        _key: Self::Key,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayout,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let formats = vec![
            Mesh::ATTRIBUTE_POSITION.format,
            ATTRIBUTE_UV.format,
            ATTRIBUTE_COLOR_BG.format,
            ATTRIBUTE_COLOR_FG.format,
        ];

        let vertex_layout =
            VertexBufferLayout::from_vertex_formats(VertexStepMode::Vertex, formats);
        descriptor.vertex.buffers = vec![vertex_layout];

        Ok(())
    }
}
