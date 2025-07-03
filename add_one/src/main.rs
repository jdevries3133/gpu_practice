use std::{
    default::Default,
    sync::{Arc, Mutex},
};
use wgpu::{
    BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BufferDescriptor, BufferUsages,
    ComputePipelineDescriptor, PipelineCompilationOptions,
    PipelineLayoutDescriptor, ShaderModuleDescriptor, ShaderStages,
    util::{BufferInitDescriptor, DeviceExt},
};

async fn run() {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
        .unwrap();
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default())
        .await
        .unwrap();

    // I've manually padded these u8's out into u32's. This helps to emphasize
    // that GPU buffers are going to be type-cast into whatever the shader
    // declares the data to be, and GPU buffers are passed to the GPU as
    // [u8], which naturally can be anything.
    let input = [
        117, 0, 0, 0, 114, 0, 0, 0, 121, 0, 0, 0, 121, 0, 0, 0, 98, 0, 0, 0,
        32, 0, 0, 0, 106, 0, 0, 0, 98, 0, 0, 0,
    ];
    let input_buffer = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("input"),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
        contents: &input,
    });
    let output_buffer_size: u64 = input.len().try_into().unwrap();
    let output_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("output"),
        size: output_buffer_size,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("main"),
        source: wgpu::ShaderSource::Wgsl(include_str!("./main.wgsl").into()),
    });
    let bind_group_layout =
        device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: None,
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
    let compute_pipeline_layout =
        device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
    let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
        label: None,
        layout: Some(&compute_pipeline_layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: PipelineCompilationOptions::default(),
        cache: None,
    });
    let bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: None,
        layout: &bind_group_layout,
        entries: &[BindGroupEntry {
            binding: 0,
            resource: input_buffer.as_entire_binding(),
        }],
    });

    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut compute_pass = encoder.begin_compute_pass(&Default::default());
        compute_pass.set_pipeline(&pipeline);
        compute_pass.set_bind_group(0, &bind_group, &[]);
        compute_pass.dispatch_workgroups(input.len() as u32, 1, 1);
    };

    // This bit confuses me. I believe that command encoder is constructing the
    // GPU pipeline. So, the correct way to interpret this command is that,
    // "the input buffer's contents will be copied into the output buffer after
    // the compute pass finishes."
    encoder.copy_buffer_to_buffer(
        &input_buffer,
        0,
        &output_buffer,
        0,
        input.len() as u64,
    );

    queue.submit(Some(encoder.finish()));

    let is_mappable = Arc::new(Mutex::new(false));
    let is_mappable_movable = is_mappable.clone();
    let buf_slice = output_buffer.slice(..);
    buf_slice.map_async(wgpu::MapMode::Read, move |result| match result {
        Ok(_) => {
            let mut handle = is_mappable_movable.lock().unwrap();
            *handle = true;
        }
        Err(e) => {
            panic!("error while mapping buffer: {e}");
        }
    });

    loop {
        loop {
            let status = device.poll(wgpu::MaintainBase::Wait).unwrap();
            if status == wgpu::PollStatus::QueueEmpty {
                break;
            }
        }
        if *is_mappable.lock().unwrap() {
            break;
        }
    }

    let output_view = &*output_buffer.get_mapped_range(..);
    dbg!(&output_view);
}

fn main() {
    env_logger::init();
    pollster::block_on(run());
}
