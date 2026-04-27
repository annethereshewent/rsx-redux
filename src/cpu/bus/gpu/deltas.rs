use crate::cpu::bus::gpu::{GPU, Polygon, Vertex};

pub struct Deltas {
    pub drdx: f32,
    pub drdy: f32,
    pub dgdx: f32,
    pub dgdy: f32,
    pub dbdx: f32,
    pub dbdy: f32,
    pub dudx: f32,
    pub dudy: f32,
    pub dvdx: f32,
    pub dvdy: f32,
}

impl Deltas {
    pub fn get_deltas(polygon: &Polygon, cp_f32: f32) -> Self {
        let (drdx_cp, drdy_cp, dgdx_cp, dgdy_cp, dbdx_cp, dbdy_cp) = if polygon.is_shaded {
            let drdx_vertices = vec![
                Vertex {
                    x: polygon.vertices[0].color.r as i32,
                    y: polygon.vertices[0].y,
                    // rest of params don't matter, as we are using the vertices to calculate cross products
                    ..Default::default()
                },
                Vertex {
                    x: polygon.vertices[1].color.r as i32,
                    y: polygon.vertices[1].y,
                    ..Default::default()
                },
                Vertex {
                    x: polygon.vertices[2].color.r as i32,
                    y: polygon.vertices[2].y,
                    ..Default::default()
                },
            ];

            let drdx_cp = GPU::cross_product(&drdx_vertices) as f32;

            let drdy_vertices = vec![
                Vertex {
                    x: polygon.vertices[0].x,
                    y: polygon.vertices[0].color.r as i32,
                    // rest of params don't matter, as we are using the vertices to calculate cross products
                    ..Default::default()
                },
                Vertex {
                    x: polygon.vertices[1].x,
                    y: polygon.vertices[1].color.r as i32,
                    // rest of params don't matter, as we are using the vertices to calculate cross products
                    ..Default::default()
                },
                Vertex {
                    x: polygon.vertices[2].x,
                    y: polygon.vertices[2].color.r as i32,
                    // rest of params don't matter, as we are using the vertices to calculate cross products
                    ..Default::default()
                },
            ];

            let drdy_cp = GPU::cross_product(&drdy_vertices) as f32;

            let dgdx_vertices = vec![
                Vertex {
                    x: polygon.vertices[0].color.g as i32,
                    y: polygon.vertices[0].y,
                    // rest of params don't matter, as we are using the vertices to calculate cross products
                    ..Default::default()
                },
                Vertex {
                    x: polygon.vertices[1].color.g as i32,
                    y: polygon.vertices[1].y,
                    ..Default::default()
                },
                Vertex {
                    x: polygon.vertices[2].color.g as i32,
                    y: polygon.vertices[2].y,
                    ..Default::default()
                },
            ];

            let dgdx_cp = GPU::cross_product(&dgdx_vertices) as f32;

            let dgdy_vertices = vec![
                Vertex {
                    x: polygon.vertices[0].x,
                    y: polygon.vertices[0].color.g as i32,
                    // rest of params don't matter, as we are using the vertices to calculate cross products
                    ..Default::default()
                },
                Vertex {
                    x: polygon.vertices[1].x,
                    y: polygon.vertices[1].color.g as i32,
                    // rest of params don't matter, as we are using the vertices to calculate cross products
                    ..Default::default()
                },
                Vertex {
                    x: polygon.vertices[2].x,
                    y: polygon.vertices[2].color.g as i32,
                    // rest of params don't matter, as we are using the vertices to calculate cross products
                    ..Default::default()
                },
            ];

            let dgdy_cp = GPU::cross_product(&dgdy_vertices) as f32;

            let dbdx_vertices = vec![
                Vertex {
                    x: polygon.vertices[0].color.b as i32,
                    y: polygon.vertices[0].y,
                    // rest of params don't matter, as we are using the vertices to calculate cross products
                    ..Default::default()
                },
                Vertex {
                    x: polygon.vertices[1].color.b as i32,
                    y: polygon.vertices[1].y,
                    ..Default::default()
                },
                Vertex {
                    x: polygon.vertices[2].color.b as i32,
                    y: polygon.vertices[2].y,
                    ..Default::default()
                },
            ];

            let dbdx_cp = GPU::cross_product(&dbdx_vertices) as f32;

            let dbdy_vertices = vec![
                Vertex {
                    x: polygon.vertices[0].x,
                    y: polygon.vertices[0].color.b as i32,
                    // rest of params don't matter, as we are using the vertices to calculate cross products
                    ..Default::default()
                },
                Vertex {
                    x: polygon.vertices[1].x,
                    y: polygon.vertices[1].color.b as i32,
                    // rest of params don't matter, as we are using the vertices to calculate cross products
                    ..Default::default()
                },
                Vertex {
                    x: polygon.vertices[2].x,
                    y: polygon.vertices[2].color.b as i32,
                    // rest of params don't matter, as we are using the vertices to calculate cross products
                    ..Default::default()
                },
            ];

            let dbdy_cp = GPU::cross_product(&dbdy_vertices) as f32;

            (drdx_cp, drdy_cp, dgdx_cp, dgdy_cp, dbdx_cp, dbdy_cp)
        } else {
            (0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
        };

        let (dudx_cp, dudy_cp, dvdx_cp, dvdy_cp) = if polygon.textured {
            let dudx_vertices = vec![
                Vertex {
                    x: polygon.vertices[0].u as i32,
                    y: polygon.vertices[0].y,
                    // rest of params don't matter, as we are using the vertices to calculate cross products
                    ..Default::default()
                },
                Vertex {
                    x: polygon.vertices[1].u as i32,
                    y: polygon.vertices[1].y,
                    ..Default::default()
                },
                Vertex {
                    x: polygon.vertices[2].u as i32,
                    y: polygon.vertices[2].y,
                    ..Default::default()
                },
            ];

            let dudx_cp = GPU::cross_product(&dudx_vertices) as f32;

            let dudy_vertices = vec![
                Vertex {
                    x: polygon.vertices[0].x,
                    y: polygon.vertices[0].u as i32,
                    // rest of params don't matter, as we are using the vertices to calculate cross products
                    ..Default::default()
                },
                Vertex {
                    x: polygon.vertices[1].x,
                    y: polygon.vertices[1].u as i32,
                    // rest of params don't matter, as we are using the vertices to calculate cross products
                    ..Default::default()
                },
                Vertex {
                    x: polygon.vertices[2].x,
                    y: polygon.vertices[2].u as i32,
                    // rest of params don't matter, as we are using the vertices to calculate cross products
                    ..Default::default()
                },
            ];

            let dudy_cp = GPU::cross_product(&dudy_vertices) as f32;

            let dvdx_vertices = vec![
                Vertex {
                    x: polygon.vertices[0].v as i32,
                    y: polygon.vertices[0].y,
                    // rest of params don't matter, as we are using the vertices to calculate cross products
                    ..Default::default()
                },
                Vertex {
                    x: polygon.vertices[1].v as i32,
                    y: polygon.vertices[1].y,
                    ..Default::default()
                },
                Vertex {
                    x: polygon.vertices[2].v as i32,
                    y: polygon.vertices[2].y,
                    ..Default::default()
                },
            ];

            let dvdx_cp = GPU::cross_product(&dvdx_vertices) as f32;

            let dvdy_vertices = vec![
                Vertex {
                    x: polygon.vertices[0].x,
                    y: polygon.vertices[0].v as i32,
                    // rest of params don't matter, as we are using the vertices to calculate cross products
                    ..Default::default()
                },
                Vertex {
                    x: polygon.vertices[1].x,
                    y: polygon.vertices[1].v as i32,
                    // rest of params don't matter, as we are using the vertices to calculate cross products
                    ..Default::default()
                },
                Vertex {
                    x: polygon.vertices[2].x,
                    y: polygon.vertices[2].v as i32,
                    // rest of params don't matter, as we are using the vertices to calculate cross products
                    ..Default::default()
                },
            ];

            let dvdy_cp = GPU::cross_product(&dvdy_vertices) as f32;

            (dudx_cp, dudy_cp, dvdx_cp, dvdy_cp)
        } else {
            (0.0, 0.0, 0.0, 0.0)
        };

        Self {
            drdx: drdx_cp / cp_f32,
            drdy: drdy_cp / cp_f32,
            dgdx: dgdx_cp / cp_f32,
            dgdy: dgdy_cp / cp_f32,
            dbdx: dbdx_cp / cp_f32,
            dbdy: dbdy_cp / cp_f32,
            dudx: dudx_cp / cp_f32,
            dudy: dudy_cp / cp_f32,
            dvdx: dvdx_cp / cp_f32,
            dvdy: dvdy_cp / cp_f32,
        }
    }
}
