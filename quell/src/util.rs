use std::{error::Error, fs::File, io::BufWriter, io::Write, path::Path, time::Duration};

use bevy::prelude::Transform;

use crate::mesh::{unrotate, unscale};

pub struct MeanCalc {
    mean: f32,
    count: u32,
}
impl MeanCalc {
    pub fn new() -> MeanCalc {
        MeanCalc {
            mean: 0.0,
            count: 0,
        }
    }

    pub fn update(&mut self, value: f32) {
        self.count += 1;
        self.mean += (value - self.mean) / self.count as f32;
    }

    pub fn update_dur(&mut self, dur: Duration) {
        let mu = dur.as_micros();
        self.update(mu as f32);
    }

    pub fn mean(&self) -> f32 {
        self.mean
    }
}

pub struct SeriesCalc {
    pub entries: Vec<f32>,
}
impl SeriesCalc {
    pub fn new() -> SeriesCalc {
        SeriesCalc {
            entries: Vec::new(),
        }
    }

    pub fn update(&mut self, value: f32) {
        self.entries.push(value);
    }

    pub fn update_dur(&mut self, dur: Duration) {
        let mu = dur.as_micros();
        self.update(mu as f32);
    }

    pub fn mean(&self) -> f32 {
        let mut sum = 0.0;
        for entry in &self.entries {
            sum += entry;
        }
        sum / self.entries.len() as f32
    }

    pub fn median(&self) -> f32 {
        let mut entries = self.entries.clone();
        entries.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mid = entries.len() / 2;
        entries[mid]
    }

    pub fn min(&self) -> f32 {
        let mut min = f32::MAX;
        for entry in &self.entries {
            if *entry < min {
                min = *entry;
            }
        }
        min
    }

    pub fn max(&self) -> f32 {
        let mut max = f32::MIN;
        for entry in &self.entries {
            if *entry > max {
                max = *entry;
            }
        }
        max
    }
}

pub fn vec_to_csv(data: &[f32], file_path: impl AsRef<Path>) -> Result<(), Box<dyn Error>> {
    let file = File::create(file_path)?;
    let mut writer = BufWriter::new(file);

    for value in data {
        writeln!(&mut writer, "{:.3}", value)?;
    }

    Ok(())
}

pub fn transform_to_vbsp(transform: Transform) -> vbsp::Vector {
    let p = transform.translation.to_array();
    // let p = unscale(p);
    let p = unrotate(p);
    vbsp::Vector {
        x: p[0],
        y: p[1],
        z: p[2],
    }
}
