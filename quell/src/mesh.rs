use bevy::{
    prelude::{AlphaMode, Color, Handle, Image, Mesh, StandardMaterial, Transform, Vec3},
    render::render_resource::PrimitiveTopology,
};
use rayon::prelude::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};
use vbsp::{Bsp, DisplacementInfo};

use crate::{data::LoadedTextures, map::GameMap};

// pub const SCALE: f32 = 0.1;
pub const SCALE: f32 = 1.0 / (1.905 * 100.0);

// TODO: use a trait or something to build the meshes so that this can be used by other libraries
// if desired.

pub fn construct_meshes<'c>(
    loaded_textures: &'c LoadedTextures,
    map: &'c GameMap,
) -> impl ParallelIterator<Item = FaceInfo<'c>> + 'c {
    // I had some trouble determining what the right way to construct the meshes early on is for
    // the map.
    // At first I did this, models -> faces
    // But then I was needing the cluster for the leaf, and tried crawling down the tree and
    // rendering
    // but as far as I can tell, the leaves aren't guaranteed to uniquely references faces?
    // So, I continue with this method, hoping it is correct.

    map.bsp
        .models
        .par_iter()
        .flat_map(|m| {
            let start = m.first_face as usize;
            let end = start + m.face_count as usize;

            map.bsp.faces[start..end]
                .par_iter()
                .enumerate()
                .map(move |(i, x)| (m, i, x))
        })
        .filter_map(move |(m, face_i, face)| {
            // TODO: do these coordinates need to be rotated?
            let origin = Vec3::new(m.origin.x, m.origin.y, m.origin.z);

            let face = vbsp::Handle::new(&map.bsp, face);
            let res = construct_face_cmd(loaded_textures, map, face, origin).transpose()?;
            // TODO: use tracing
            match res {
                Ok(mut face_info) => {
                    face_info.face_i = face_i;
                    Some(face_info)
                }
                Err(err) => {
                    eprintln!("Failed to construct face: {:?}", err);
                    None
                }
            }
        })
}

// pub fn construct_meshes<'c>(
//     loaded_textures: &'c LoadedTextures,
//     map: &'c GameMap,
// ) -> impl ParallelIterator<Item = FaceInfo> + 'c {
//     // Rather than crawling the node tree like typical, we just don't do that and iterate over all
//     // the nodes in the map. This lets us just process them entirely in parallel.
//     // However, this does make it marginally harder to mape certain things back to their parents,
//     // but I'm not sure we want to replicate the bsp tree in the bevy's ECS anyway.

//     map.bsp
//         .leaves
//         .par_iter()
//         .flat_map(|leaf| {
//             let start = leaf.first_leaf_face as usize;
//             let end = start + leaf.leaf_face_count as usize;

//             map.bsp.leaf_faces[start..end]
//                 .par_iter()
//                 .map(|x| &map.bsp.faces[usize::from(x.face)])
//                 .map(move |x| (leaf, x))
//         })
//         .filter_map(move |(leaf, face)| {
//             // TODO: do we need to use the `Model` origin? ctf_2fort had all of them as 0,0,0..

//             // Cluster identifies which part of the map it is in for easy visibility computation
//             let cluster = leaf.cluster;

//             let origin = Vec3::ZERO;

//             let face = vbsp::Handle::new(&map.bsp, face);
//             let res = construct_face_cmd(loaded_textures, map, face, origin).transpose()?;
//             // TODO: use tracing
//             match res {
//                 Ok(mut face_info) => {
//                     face_info.cluster = cluster;
//                     Some(face_info)
//                 }
//                 Err(err) => {
//                     eprintln!("Failed to construct face: {:?}", err);
//                     None
//                 }
//             }
//         })
// }

#[derive(Debug, Clone)]
pub struct FaceInfo<'a> {
    pub mesh: Mesh,
    pub material_name: &'a str,
    pub transform: Transform,
    pub face_i: usize,
}

/// Construct the information needed to create a face.
/// This currently expects any textures to already be loaded so that it can easily be used in
/// parallel.
fn construct_face_cmd<'a>(
    loaded_textures: &LoadedTextures,
    map: &'a GameMap,
    face: vbsp::Handle<'a, vbsp::Face>,
    offset: Vec3,
) -> eyre::Result<Option<FaceInfo<'a>>> {
    let texture_info = face.texture();
    let texture_data = texture_info.texture_data();

    // TODO: create nodraw meshes but hide them so we can render them in debug mode
    // TODO: create the skybox
    if texture_info.flags.contains(vbsp::TextureFlags::NODRAW)
        || texture_info.flags.contains(vbsp::TextureFlags::SKY)
    {
        return Ok(None);
    }

    let texture_name = texture_info.name();

    // TODO: this probably ignores other pieces that we shouldn't render
    if texture_name.eq_ignore_ascii_case("tools/toolstrigger") {
        return Ok(None);
    }

    let reflect = texture_data.reflectivity;
    let color = if texture_info.flags.contains(vbsp::TextureFlags::SKY) {
        Color::rgb(0.0, 0.0, 0.0)
    } else {
        let alpha = if texture_info.flags.contains(vbsp::TextureFlags::TRANS) {
            0.2
        } else {
            1.0
        };

        // TODO: actually just get this texture
        if texture_name == "water/water_2fort_beneath.vmt" {
            Color::rgba(0.0, 0.0, 0.8, 0.4)
        } else {
            Color::rgba(reflect.x, reflect.y, reflect.z, alpha)
        }
    };

    if let Some(disp) = face.displacement() {
        Ok(Some(create_displacement_mesh(
            &map.bsp, face, disp, offset, color,
        )))
    } else {
        let texture_name = texture_info.name();
        let texture = match loaded_textures.find_material_texture(texture_name) {
            Some(Ok(texture)) => texture,
            Some(Err(err)) => {
                eprintln!("Failed to load texture: {:?}", err);
                loaded_textures.missing_texture.clone()
            }
            None => {
                eprintln!("Missing texture: {:?}", texture_name);
                loaded_textures.missing_texture.clone()
            }
        };

        Ok(Some(create_basic_map_mesh(
            &map.bsp,
            face,
            offset,
            color,
            Some(texture),
        )))
    }
}

// TODO: we don't create the map mesh with a transform yet it is positioned fine, so probably the
// triangles are being positioned 'manually'. I think we should maybe try getting them to center on
// 0,0,0 and then apply a transform to make it work nicer with other transform stuff
fn create_basic_map_mesh<'a>(
    bsp: &'a Bsp,
    face: vbsp::Handle<'a, vbsp::Face>,
    offset: Vec3,
    color: Color,
    texture: Option<Handle<Image>>,
) -> FaceInfo<'a> {
    let texture_info = face.texture();
    let tex_width = texture_info.texture().width as f32;
    let tex_height = texture_info.texture().height as f32;

    let normal = if texture_info.flags.contains(vbsp::TextureFlags::SKY) {
        [0.0, 0.0, 1.0]
    } else {
        let plane = bsp.planes.get(face.plane_num as usize).unwrap();
        plane.normal.into()
    };
    let normal = rotate(normal);
    let normal = [-normal[0], -normal[2], -normal[2]];
    // let normal = [-normal[0], -normal[2], -normal[1]];
    // TODO: do we need to rotate the normal?

    // TODO(minor): preallocate
    let mut face_triangles = Vec::new();
    let mut face_normals = Vec::new();
    let mut face_uvs = Vec::new();

    let mut triangle_vert = 0;
    let mut triangle = [[0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]];
    for i in 0..face.num_edges {
        let surface_edge = bsp
            .surface_edges
            .get((face.first_edge + i as i32) as usize)
            .unwrap();
        let edge = bsp.edges.get(surface_edge.edge_index() as usize).unwrap();
        let vertex_index = match surface_edge.direction() {
            vbsp::EdgeDirection::FirstToLast => edge.start_index,
            vbsp::EdgeDirection::LastToFirst => edge.end_index,
        };

        let vertex = bsp.vertices.get(vertex_index as usize).unwrap();
        let vertex = <[f32; 3]>::from(vertex.position);
        let vertex = scale(vertex);
        let vertex = rotate(vertex);

        triangle[triangle_vert] = vertex;
        triangle_vert += 1;

        if triangle_vert > 2 {
            // TODO: I swapped the order of these because my rotate also made the z neg
            // and that seems to fix things, but I don't completely understand the details
            let vert = triangle[2];
            face_triangles.push(vert);
            face_normals.push(normal);
            face_uvs.push(calc_uv(&texture_info, vert, tex_width, tex_height));

            let vert = triangle[1];
            face_triangles.push(vert);
            face_normals.push(normal);
            face_uvs.push(calc_uv(&texture_info, vert, tex_width, tex_height));

            let vert = triangle[0];
            face_triangles.push(vert);
            face_normals.push(normal);
            face_uvs.push(calc_uv(&texture_info, vert, tex_width, tex_height));

            triangle[1] = triangle[2];
            triangle_vert = 2;
        }
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, face_triangles);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, face_normals);
    // panic!();
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, face_uvs);
    // TODO: lightmaps with UV_1?

    FaceInfo {
        mesh,
        transform: Transform::from_translation(offset),
        material_name: texture_info.name(),
        // TODO: do something better than letting the caller set this?
        face_i: 0,
    }
}

/// Calculate the UV coordinates for the given vertex and texture.
fn calc_uv(
    texture_info: &vbsp::TextureInfo,
    vertex: [f32; 3],
    tex_width: f32,
    tex_height: f32,
) -> [f32; 2] {
    // [xmul, ymul, zmul, offset]
    let scale = texture_info.texture_scale;
    let transform = texture_info.texture_transform;

    // Undo the scaling
    let vertex = [vertex[0] / SCALE, vertex[1] / SCALE, vertex[2] / SCALE];
    // Convert to texture coordinates (y-down)
    let vertex = tex_coord(vertex);

    // Convert from z-up to y-up, and then to texture coordinates
    let scale = tex_coord_4(rotate_4(scale));
    let transform = tex_coord_4(rotate_4(transform));

    // xmul * x + ymul * y + zmul * z + offset
    let u = scale[0] * vertex[0] + scale[1] * vertex[1] + scale[2] * vertex[2] + scale[3];
    let v = transform[0] * vertex[0]
        + transform[1] * vertex[1]
        + transform[2] * vertex[2]
        + transform[3];

    // Normalize by the texture size
    let u = u / tex_width;
    let v = v / tex_height;

    [u, v]
}

fn create_displacement_mesh<'a>(
    bsp: &'a vbsp::Bsp,
    face: vbsp::Handle<'a, vbsp::Face>,
    disp: vbsp::Handle<'a, DisplacementInfo>,
    offset: Vec3,
    color: Color,
) -> FaceInfo<'a> {
    let low_base = disp.start_position; // * SCALE;
    let low_base = <[f32; 3]>::from(low_base);
    // let low_base = rotate(low_base);

    if face.num_edges != 4 {
        panic!("Bad displacement!\n");
    }

    let mut corner_verts = [[0.0, 0.0, 0.0]; 4];
    let mut base_i = None;
    let mut base_dist = std::f32::INFINITY;
    for (i, corner_vert) in corner_verts.iter_mut().enumerate() {
        let surface_edge = bsp
            .surface_edges
            .get((face.first_edge + i as i32) as usize)
            .unwrap();
        let edge = bsp.edges.get(surface_edge.edge_index() as usize).unwrap();
        let vertex_index = match surface_edge.direction() {
            vbsp::EdgeDirection::FirstToLast => edge.start_index,
            vbsp::EdgeDirection::LastToFirst => edge.end_index,
        };

        let vertex = bsp.vertices.get(vertex_index as usize).unwrap();
        let vertex = <[f32; 3]>::from(vertex.position);
        // let vertex = [vertex[0] * SCALE, vertex[1] * SCALE, vertex[2] * SCALE];
        // let vertex = rotate(vertex);

        *corner_vert = vertex;

        let this_dist = (vertex[0] - low_base[0]).abs()
            + (vertex[2] - low_base[2]).abs()
            + (vertex[1] - low_base[1]).abs();
        if this_dist < base_dist {
            base_dist = this_dist;
            base_i = Some(i);
        }
    }

    let base_i = base_i.expect("Bad base in displacement");

    let high_base = corner_verts[(base_i + 3) % 4];
    let high_ray = corner_verts[(base_i + 2) % 4];
    let high_ray = [
        high_ray[0] - high_base[0],
        high_ray[1] - high_base[1],
        high_ray[2] - high_base[2],
    ];
    let low_ray = corner_verts[(base_i + 1) % 4];
    let low_ray = [
        low_ray[0] - low_base[0],
        low_ray[1] - low_base[1],
        low_ray[2] - low_base[2],
    ];

    let verts_wide = (2 << (disp.power - 1)) + 1;

    let mut base_verts = vec![[0.0, 0.0, 0.0]; 289];
    let mut base_alphas = vec![0.0; 289];

    for y in 0..verts_wide {
        let fy = y as f32 / (verts_wide as f32 - 1.0);

        let mid_base = [
            low_base[0] + low_ray[0] * fy,
            low_base[1] + low_ray[1] * fy,
            low_base[2] + low_ray[2] * fy,
        ];
        let mid_ray = [
            high_base[0] + high_ray[0] * fy - mid_base[0],
            high_base[1] + high_ray[1] * fy - mid_base[1],
            high_base[2] + high_ray[2] * fy - mid_base[2],
        ];

        for x in 0..verts_wide {
            let fx = x as f32 / (verts_wide as f32 - 1.0);
            let i = x + y * verts_wide;

            // TODO: use disp.displacement_vertices
            let disp_vert = bsp
                .displacement_vertices
                .get((disp.displacement_vertex_start + i) as usize)
                .unwrap();
            let offset = <[f32; 3]>::from(disp_vert.vector);
            let scale = disp_vert.distance;
            let alpha = disp_vert.alpha;

            base_verts[i as usize] = [
                mid_base[0] + mid_ray[0] * fx + offset[0] * scale,
                mid_base[1] + mid_ray[1] * fx + offset[1] * scale,
                mid_base[2] + mid_ray[2] * fx + offset[2] * scale,
            ];
            base_alphas[i as usize] = alpha;
        }
    }

    let mut tris = Vec::new();
    let mut normals = Vec::new();

    for y in 0..(verts_wide - 1) {
        for x in 0..(verts_wide - 1) {
            let i = x + y * verts_wide;

            let v1 = scale(rotate(base_verts[i as usize]));
            let v2 = scale(rotate(base_verts[(i + 1) as usize]));
            let v3 = scale(rotate(base_verts[(i + verts_wide) as usize]));
            let v4 = scale(rotate(base_verts[(i + verts_wide + 1) as usize]));

            // TODO: I'm unsure about the normal calculations. I think they were originally done in
            // the source or opengl coordinates rather than bevys and not sure I corrected them
            // right.
            if i % 2 != 0 {
                let normal = find_normal(v2, v3, v1);
                // let color = Color::rgb(tex_r1, tex_g1, tex_b1);

                tris.push(v2);
                normals.push(normal);
                tris.push(v3);
                normals.push(normal);
                tris.push(v1);
                normals.push(normal);

                let normal = find_normal(v4, v3, v2);

                tris.push(v4);
                normals.push(normal);
                tris.push(v3);
                normals.push(normal);
                tris.push(v2);
                normals.push(normal);
            } else {
                let normal = find_normal(v4, v3, v1);

                tris.push(v4);
                normals.push(normal);
                tris.push(v3);
                normals.push(normal);
                tris.push(v1);
                normals.push(normal);

                let normal = find_normal(v4, v1, v2);

                tris.push(v4);
                normals.push(normal);
                tris.push(v1);
                normals.push(normal);
                tris.push(v2);
                normals.push(normal);
            }
        }
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, tris);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);

    FaceInfo {
        mesh,
        transform: Transform::from_translation(offset),
        material_name: face.texture().name(),
        // TODO: do something better than letting the caller set this?
        face_i: 0,
    }
}

fn find_normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    let u = [b[0] - c[0], b[1] - c[1], b[2] - c[2]];
    let v = [a[0] - c[0], a[1] - c[1], a[2] - c[2]];

    let norm = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];

    let len = (norm[0] * norm[0] + norm[1] * norm[1] + norm[2] * norm[2]).sqrt();

    [norm[0] / len, norm[1] / len, norm[2] / len]
}

// fn pick_color(name: &str, x: f32) -> u32 {
//     // TODO: more colors
//     let col = 77550;

//     col
// }

/// Rotate from a source engine vector to a bevy vector.
pub fn rotate(v: [f32; 3]) -> [f32; 3] {
    [-v[1], v[2], -v[0]]
}
/// Rotate from a source engine vector to a bevy vector.
pub fn rotate_4(v: [f32; 4]) -> [f32; 4] {
    [-v[1], v[2], -v[0], v[3]]
}

/// Rotate from a bevy vector to a source engine vector.
pub fn unrotate(v: [f32; 3]) -> [f32; 3] {
    [-v[2], -v[0], v[1]]
}

pub fn scale(v: [f32; 3]) -> [f32; 3] {
    [v[0] * SCALE, v[1] * SCALE, v[2] * SCALE]
}
pub fn unscale(v: [f32; 3]) -> [f32; 3] {
    [v[0] / SCALE, v[1] / SCALE, v[2] / SCALE]
}
/// Convert a y-up (bevy) vector to a tex coordinate vector
pub(crate) fn tex_coord(v: [f32; 3]) -> [f32; 3] {
    [v[0], -v[1], v[2]]
}
pub(crate) fn tex_coord_4(v: [f32; 4]) -> [f32; 4] {
    [v[0], -v[1], v[2], v[3]]
}

pub fn angle_map(a: [f32; 3]) -> [f32; 3] {
    let a = rotate(a);
    // TODO: this might not work if we allow negative angles?
    // let a = [a[0].min(90.), a[1].min(90.0), a[2].min(90.0)];
    a
}

pub fn degrees_to_radians(degrees: f32) -> f32 {
    degrees * (std::f32::consts::PI / 180.0)
}
