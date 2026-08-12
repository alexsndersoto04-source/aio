//! Numerically stable streaming descriptive statistics.

#[derive(Debug, Clone, Default)]
pub struct Summary {
    count: u64,
    mean: f64,
    m2: f64,
    min: f64,
    max: f64,
}
impl Summary {
    pub fn new() -> Self {
        Self {
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
            ..Self::default()
        }
    }
    pub fn push(&mut self, value: f64) {
        if !value.is_finite() {
            return;
        }
        self.count += 1;
        let delta = value - self.mean;
        self.mean += delta / self.count as f64;
        self.m2 += delta * (value - self.mean);
        self.min = self.min.min(value);
        self.max = self.max.max(value);
    }
    pub fn extend(&mut self, values: impl IntoIterator<Item = f64>) {
        for value in values {
            self.push(value);
        }
    }
    pub fn count(&self) -> u64 {
        self.count
    }
    pub fn mean(&self) -> Option<f64> {
        (self.count > 0).then_some(self.mean)
    }
    pub fn variance_population(&self) -> Option<f64> {
        (self.count > 0).then_some(self.m2 / self.count as f64)
    }
    pub fn variance_sample(&self) -> Option<f64> {
        (self.count > 1).then_some(self.m2 / (self.count - 1) as f64)
    }
    pub fn standard_deviation(&self) -> Option<f64> {
        self.variance_population().map(f64::sqrt)
    }
    pub fn min(&self) -> Option<f64> {
        (self.count > 0).then_some(self.min)
    }
    pub fn max(&self) -> Option<f64> {
        (self.count > 0).then_some(self.max)
    }
}
pub fn median(values: &mut [f64]) -> Option<f64> {
    quantile(values, 0.5)
}
pub fn quantile(values: &mut [f64], q: f64) -> Option<f64> {
    if values.is_empty() || !(0.0..=1.0).contains(&q) || values.iter().any(|v| !v.is_finite()) {
        return None;
    }
    values.sort_by(f64::total_cmp);
    let position = q * (values.len() - 1) as f64;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    let fraction = position - lower as f64;
    Some(values[lower] * (1.0 - fraction) + values[upper] * fraction)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stable_summary() {
        let mut summary = Summary::new();
        summary.extend([1.0, 2.0, 3.0, 4.0]);
        assert_eq!(summary.mean(), Some(2.5));
        assert_eq!(summary.variance_population(), Some(1.25));
        let mut values = [4.0, 1.0, 3.0, 2.0];
        assert_eq!(median(&mut values), Some(2.5));
    }
}
