// Todo: 座標の直打ちやめろ

extern crate rand;
use rand::Rng;
use rapier2d::prelude::*;
use std::collections::HashMap;
use super::canvas;

pub use rapier2d::prelude::Real;

#[derive(Debug, Clone)]
pub struct Object {
    pub user_id: Option<String>,
    pub shape: Vec<(f64, f64)>,
    pub translation: Vector<Real>,
    pub rotation: Real,
    rigid_body_handle: RigidBodyHandle,
}

impl Object {
    pub fn get_top(&self) -> Real {
        let mut top = Real::MAX;
        for vertex in &self.shape {
            let point1 = Point::new(vertex.0 as Real, vertex.1 as Real);
            let point2 = Point::new(
                point1.x * self.rotation.cos() - point1.y * self.rotation.sin() + self.translation.x,
                point1.x * self.rotation.sin() + point1.y * self.rotation.cos() + self.translation.y,
            );
            if top > point2.y { top = point2.y; }
        }
        return top;
    }

    pub fn get_radius(&self) -> Real {
        let mut radius: Real = 0.0;
        for vertex in &self.shape {
            let radius_ = (vertex.0 * vertex.0 + vertex.1 * vertex.1).sqrt();
            if radius_ as Real > radius { radius = radius_ as Real; }
        }
        return radius;
    }
}

pub struct Stage {
    pub user_icons: HashMap<String, Vec<u8>>,

    // Rapier 2D
    world_scale: Real,
    gravity: Vector<Real>,
    integration_parameters: IntegrationParameters,
    physics_pipeline: PhysicsPipeline,
    island_manager: IslandManager,
    broad_phase: BroadPhase,
    narrow_phase: NarrowPhase,
    rigid_body_set: RigidBodySet,
    collider_set: ColliderSet,
    impulse_joint_set: ImpulseJointSet,
    multibody_joint_set: MultibodyJointSet,
    ccd_solver: CCDSolver,
    physics_hooks: (),
    event_handler: (),

    // Game Objects
    objects: Vec<Object>,
    shapes: Vec<Vec<(f64, f64)>>,
}

#[derive(PartialEq, Debug)]
pub enum TurnResult {
    Success,
    Failure,
    Timeout,
}

impl Stage {
    pub fn new(shapes: Vec<Vec<(f64, f64)>>) -> Self {
        let mut stage = Stage {
            user_icons: HashMap::new(),

            // Rapier 2D
            world_scale: 0.01,
            gravity: vector![0.0, 9.81],
            integration_parameters: IntegrationParameters::default(),
            physics_pipeline: PhysicsPipeline::new(),
            island_manager: IslandManager::new(),
            broad_phase: BroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            rigid_body_set: RigidBodySet::new(),
            collider_set: ColliderSet::new(),
            impulse_joint_set: ImpulseJointSet::new(),
            multibody_joint_set: MultibodyJointSet::new(),
            ccd_solver: CCDSolver::new(),
            physics_hooks: (),
            event_handler: (),

            // Game Object Handles
            objects: Vec::new(),
            shapes,
        };

        // 地面の生成
        let collider =
            ColliderBuilder::cuboid(220.0 * stage.world_scale, 10.0 * stage.world_scale)
                .translation(vector![320.0 * stage.world_scale, 410.0 * stage.world_scale])
                .build();
        stage.collider_set.insert(collider);

        return stage;
    }

    pub fn next_turn(
        &mut self,
        user_id: Option<String>,
        translation_x: Real, rotation: Real,
    ) -> Result<(TurnResult, Real, Vec<u8>), Box<dyn std::error::Error + Send + Sync + 'static>> {
        self.reset_last_object(user_id, translation_x, rotation);
        let turn_result = self.continue_until_convergence(60.0);
        let height = self.get_stage_height();
        if TurnResult::Success == turn_result { self.add_object(); }
        let data = self.render_frame()?;
        Ok((turn_result, height, data))
    }

    fn add_object(&mut self) {
        let shape = &self.shapes[rand::thread_rng().gen_range(0..self.shapes.len())];
        let mut vertices = Vec::<Point<Real>>::new();
        let mut indices = Vec::<[u32; DIM]>::new();
        for (index, vertex) in shape.iter().enumerate() {
            vertices.push(Point::new(vertex.0 as Real * self.world_scale, vertex.1 as Real * self.world_scale));
            if index == shape.len() - 1 {
                indices.push([index as u32, 0]);
            }
            else {
                indices.push([index as u32, index as u32 + 1]);
            }
        }

        let rigid_body = RigidBodyBuilder::dynamic()
            .build();
        let collider = ColliderBuilder::convex_decomposition(&vertices, &indices).friction(1.0).build();
        let shape_body_handle = self.rigid_body_set.insert(rigid_body);
        self.collider_set.insert_with_parent(collider, shape_body_handle, &mut self.rigid_body_set);
        let mut object = Object{
            user_id: None,
            shape: shape.clone(),
            translation: vector![0.0, 0.0],
            rotation: 0.0,
            rigid_body_handle: shape_body_handle
        };
        let translation = vector![0.0, self.get_stage_top() - object.get_radius() - 50.0];
        object.translation = translation;
        self.objects.push(object);
        self.reset_last_object(None, 0.0, 0.0);
    }

    fn reset_last_object(&mut self, user_id: Option<String>, translation_x: Real, rotation: Real) {
        if let Some(object) = self.objects.last_mut() {
            object.user_id = user_id;
            object.translation.x = ((translation_x + 1.0) * 0.5 * 640.0) as Real;
            object.rotation = rotation.to_radians() as Real;
            self.rigid_body_set[object.rigid_body_handle].set_position(Isometry::new(object.translation * self.world_scale, object.rotation), true);
        }
    }

    fn continue_until_convergence(&mut self, timeout_sec: Real) -> TurnResult {
        // timeout_sec秒まで物理演算を実行
        let timeout_frame = (timeout_sec / self.integration_parameters.dt).floor() as u64;
        for _ in 0..timeout_frame {
            self.physics_pipeline.step(
                &self.gravity,
                &self.integration_parameters,
                &mut self.island_manager,
                &mut self.broad_phase,
                &mut self.narrow_phase,
                &mut self.rigid_body_set,
                &mut self.collider_set,
                &mut self.impulse_joint_set,
                &mut self.multibody_joint_set,
                &mut self.ccd_solver,
                &self.physics_hooks,
                &self.event_handler,
            );

            for object in &mut self.objects {
                let body = &self.rigid_body_set[object.rigid_body_handle];
                let rotation = body.rotation();
                object.translation = body.translation() / self.world_scale;
                object.rotation = rotation.im.atan2(rotation.re);
            }

            // オブジェクトが地面から1つでも落下した場合は失敗判定
            for object in &self.objects {
                let obj_top = object.get_top() * self.world_scale;
                if obj_top > 420.0 * self.world_scale { return TurnResult::Failure; }
            }

            // オブジェクトが全て静止した場合は成功判定
            // メモ: 要素数0のall()はtrueを返す
            let all_sleep = self.objects.iter().all(|object| self.rigid_body_set[object.rigid_body_handle].is_sleeping());
            if all_sleep { return TurnResult::Success; }
        }

        // オブジェクトが全て静止しなかった場合はタイムアウト判定
        return TurnResult::Timeout;
    }

    fn render_frame(&self) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync + 'static>> {
        let mut canvas = canvas::Canvas::new(640.0, 480.0);

        let top: f64 = 0.0f64.min(self.get_stage_top() as f64 - 10.0);

        for (user_id, user_icon) in self.user_icons.iter() {
            canvas.add_image(user_id.clone(), &user_icon);
        }

        canvas.set_no_stroke();
        canvas.set_color_fill(3, 182, 252);
        canvas.add_shape(&vec![
            (  0.0,   0.0),
            (640.0,   0.0),
            (640.0, 480.0),
            (  0.0, 480.0),
        ], (0.0, 0.0), 0.0);
        canvas.set_color_fill(20, 222, 106);
        canvas.add_shape(&vec![
            (100.0, 400.0 - top),
            (540.0, 400.0 - top),
            (540.0, 420.0 - top),
            (100.0, 420.0 - top),
        ], (0.0, 0.0), 0.0);

        for object in &self.objects {
            canvas.set_color_fill(255, 255, 255);
            canvas.set_color_stroke(245, 66, 129, 4.0);
            if let Some(user_id) = &object.user_id {
                if self.user_icons.contains_key(user_id) {
                    canvas.set_image_fill(user_id.clone());
                    canvas.set_color_stroke(0, 88, 122, 2.0);
                }
            }
            canvas.add_shape(&object.shape, (object.translation.x as f64, object.translation.y as f64 - top), object.rotation.to_degrees() as f64);
        }

        let data = canvas.encode_png()?;

        Ok(data)
    }

    fn get_stage_top(&self) -> Real {
        let mut top = 420.0;
        for object in &self.objects {
            let obj_top = object.get_top();
            if top > obj_top { top = obj_top; }
        }
        return top;
    }

    fn get_stage_height(&self) -> Real {
        return (420.0 - self.get_stage_top()) * self.world_scale;
    }
}
