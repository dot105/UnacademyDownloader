// =====================[ MATHS ]=====================
pub fn mul_items(items: [f64; 2], m: f64) -> [f64; 2] { [items[0] * m, items[1] * m] }
pub fn mul_matrix(m1: [f64; 2], m2: [f64; 2]) -> f64 { m1[0] * m2[0] + m1[1] * m2[1] }
pub fn subtract(a1: [f64; 2], a2: [f64; 2]) -> [f64; 2] { [a1[0] - a2[0], a1[1] - a2[1]] }
pub fn add_arrays(a1: [f64; 2], a2: [f64; 2]) -> [f64; 2] { [a1[0] + a2[0], a1[1] + a2[1]] }
pub fn add_items(items: [f64; 2], a: f64) -> [f64; 2] { [items[0] + a, items[1] + a] }
pub fn sum(items: [f64; 2]) -> f64 { items[0] + items[1] }
pub fn dot(m1: [f64; 2], m2: [f64; 2]) -> f64 { m1[0] * m2[0] + m1[1] * m2[1] }
pub fn vector_len(v: [f64; 2]) -> f64 { (v[0] * v[0] + v[1] * v[1]).sqrt() }
pub fn div_items(items: [f64; 2], d: f64) -> [f64; 2] { [items[0] / d, items[1] / d] }
pub fn square_items(items: [f64; 2]) -> [f64; 2] { [items[0] * items[0], items[1] * items[1]] }
pub fn normalize(v: [f64; 2]) -> [f64; 2] { div_items(v, vector_len(v)) }
pub fn euclidean_distance(a: (f64, f64), b: (f64, f64)) -> f64 { ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt() }
// =====================[ BEZIER ]=====================
pub fn q(ctrl: [[f64; 2]; 4], t: f64) -> [f64; 2] {
    let tx = 1.0 - t; let p_a = mul_items(ctrl[0], tx * tx * tx); let p_b = mul_items(ctrl[1], 3.0 * tx * tx * t); let p_c = mul_items(ctrl[2], 3.0 * tx * t * t); let p_d = mul_items(ctrl[3], t * t * t); add_arrays(add_arrays(p_a, p_b), add_arrays(p_c, p_d))
}
pub fn qprime(ctrl: [[f64; 2]; 4], t: f64) -> [f64; 2] {
    let tx = 1.0 - t; let p_a = mul_items(subtract(ctrl[1], ctrl[0]), 3.0 * tx * tx); let p_b = mul_items(subtract(ctrl[2], ctrl[1]), 6.0 * tx * t); let p_c = mul_items(subtract(ctrl[3], ctrl[2]), 3.0 * t * t); add_arrays(add_arrays(p_a, p_b), p_c)
}
pub fn qprimeprime(ctrl: [[f64; 2]; 4], t: f64) -> [f64; 2] {
    add_arrays(mul_items(add_arrays(subtract(ctrl[2], mul_items(ctrl[1], 2.0)), ctrl[0]), 6.0 * (1.0 - t)), mul_items(add_arrays(subtract(ctrl[3], mul_items(ctrl[2], 2.0)), ctrl[1]), 6.0 * t))
}
// =====================[ FIT CURVE ]=====================
pub struct FitResult { pub b: Vec<[[f64; 2]; 4]>, pub m: Vec<(f64, f64, f64)> }
pub fn fit_curve(points: &[(f64, f64, f64)], max_error: f64) -> FitResult {
    if points.len() < 2 { return FitResult { b: vec![], m: points.iter().map(|p| (p.0, p.1, p.2)).collect() } }
    let len = points.len(); let left = create_tangent([points[1].0, points[1].1], [points[0].0, points[0].1]); let right = create_tangent([points[len - 2].0, points[len - 2].1], [points[len - 1].0, points[len - 1].1]); fit_cubic(points, left, right, max_error)
}
fn create_tangent(a: [f64; 2], b: [f64; 2]) -> [f64; 2] { let t = normalize(subtract(a, b)); if t[0].is_nan() || t[1].is_nan() { [0.0, 0.0] } else { t } }
fn fit_cubic(points: &[(f64, f64, f64)], left: [f64; 2], right: [f64; 2], error: f64) -> FitResult {
    if points.len() < 2 { return FitResult { b: vec![], m: points.iter().map(|p| (p.0, p.1, p.2)).collect() } }
    let max_iter = 20;
    if points.len() == 2 {
        let dist = vector_len(subtract([points[0].0, points[0].1], [points[1].0, points[1].1])) / 3.0;
        let curve = [[points[0].0, points[0].1], add_arrays([points[0].0, points[0].1], mul_items(left, dist)), add_arrays([points[1].0, points[1].1], mul_items(right, dist)), [points[1].0, points[1].1]];
        return FitResult { b: vec![curve], m: points.iter().map(|p| (p.0, p.1, p.2)).collect() }
    }
    let mut u = chord_length_parameterize(points);
    let (mut curve, mut max_err, mut split, mut map) = generate_and_report(points, &u, &u, left, right);
    if max_err < error { return FitResult { b: vec![curve], m: map } }
    if max_err < error * error {
        let mut prev_err = max_err; let mut prev_split = split;
        for _ in 0..max_iter {
            u = reparameterize(&curve, points, &u);
            let (c, e, s, m) = generate_and_report(points, &u, &u, left, right);
            curve = c; max_err = e; split = s; map = m;
            if max_err < error { return FitResult { b: vec![curve], m: map } }
            if split == prev_split { let change = max_err / prev_err; if change > 0.9999 && change < 1.0001 { break } }
            prev_err = max_err; prev_split = split;
        }
    }
    let (to_center, from_center) = get_split_tangents(points, split);
    let left_result = fit_cubic(&points[..split + 1], left, to_center, error);
    let right_result = fit_cubic(&points[split..], from_center, right, error);
    let mut curves = left_result.b; curves.extend(right_result.b);
    let mut map = left_result.m; map.pop(); map.extend(right_result.m);
    FitResult { b: curves, m: map }
}
fn get_split_tangents(points: &[(f64, f64, f64)], split: usize) -> ([f64; 2], [f64; 2]) {
    let mut center = subtract([points[split - 1].0, points[split - 1].1], [points[split + 1].0, points[split + 1].1]);
    if center[0] == 0.0 && center[1] == 0.0 { center = subtract([points[split - 1].0, points[split - 1].1], [points[split].0, points[split].1]); center[0] = -center[0] }
    let to_center = normalize(center); (to_center, mul_items(to_center, -1.0))
}
fn generate_and_report(points: &[(f64, f64, f64)], params_orig: &[f64], params_prime: &[f64], left: [f64; 2], right: [f64; 2]) -> ([[f64; 2]; 4], f64, usize, Vec<(f64, f64, f64)>) {
    let curve = generate_bezier(points, params_prime, left, right);
    let (max_err, split, map) = compute_max_error(points, &curve, params_orig);
    (curve, max_err, split, map)
}
fn generate_bezier(points: &[(f64, f64, f64)], params: &[f64], left: [f64; 2], right: [f64; 2]) -> [[f64; 2]; 4] {
    let first = [points[0].0, points[0].1]; let last = [points[points.len() - 1].0, points[points.len() - 1].1];
    let mut a = vec![[[0.0f64; 2]; 2]; params.len()];
    for (i, &u) in params.iter().enumerate() { let ux = 1.0 - u; a[i][0] = mul_items(left, 3.0 * u * (ux * ux)); a[i][1] = mul_items(right, 3.0 * ux * (u * u)); }
    let mut c = [[0.0f64; 2]; 2]; let mut x = [0.0f64; 2];
    for (i, &u) in params.iter().enumerate() {
        c[0][0] += dot(a[i][0], a[i][0]); c[0][1] += dot(a[i][0], a[i][1]); c[1][0] += dot(a[i][0], a[i][1]); c[1][1] += dot(a[i][1], a[i][1]);
        let tmp = subtract([points[i].0, points[i].1], q([first, first, last, last], u)); x[0] += dot(a[i][0], tmp); x[1] += dot(a[i][1], tmp);
    }
    let det_c0_c1 = c[0][0] * c[1][1] - c[1][0] * c[0][1]; let det_c0_x = c[0][0] * x[1] - c[1][0] * x[0]; let det_x_c1 = x[0] * c[1][1] - x[1] * c[0][1];
    let alpha_l = if det_c0_c1 == 0.0 { 0.0 } else { det_x_c1 / det_c0_c1 }; let alpha_r = if det_c0_c1 == 0.0 { 0.0 } else { det_c0_x / det_c0_c1 };
    let seg_len = vector_len(subtract(first, last)); let eps = 1.0e-6 * seg_len;
    if alpha_l < eps || alpha_r < eps { return [first, add_arrays(first, mul_items(left, seg_len / 3.0)), add_arrays(last, mul_items(right, seg_len / 3.0)), last] }
    [first, add_arrays(first, mul_items(left, alpha_l)), add_arrays(last, mul_items(right, alpha_r)), last]
}
fn reparameterize(bez: &[[f64; 2]; 4], points: &[(f64, f64, f64)], params: &[f64]) -> Vec<f64> { params.iter().enumerate().map(|(i, &p)| newton_raphson(bez, [points[i].0, points[i].1], p)).collect() }
fn newton_raphson(bez: &[[f64; 2]; 4], point: [f64; 2], u: f64) -> f64 {
    let d = subtract(q(bez, u), point); let qp = qprime(bez, u);
    let num = mul_matrix(d, qp); let den = sum(add_items(square_items(qp), mul_matrix(d, qprimeprime(bez, u))));
    if den == 0.0 { u } else { u - num / den }
}
fn chord_length_parameterize(points: &[(f64, f64, f64)]) -> Vec<f64> {
    let mut u = vec![0.0]; let mut prev = [points[0].0, points[0].1]; let mut total = 0.0;
    for p in &points[1..] { let cur = [p.0, p.1]; total += vector_len(subtract(cur, prev)); u.push(total); prev = cur; }
    u.iter().map(|x| if total > 0.0 { x / total } else { 0.0 }).collect()
}
fn compute_max_error(points: &[(f64, f64, f64)], bez: &[[f64; 2]; 4], params: &[f64]) -> (f64, usize, Vec<(f64, f64, f64)>) {
    let mut max_dist = 0.0; let mut split = points.len() / 2; let mut map = Vec::new();
    if split < 1 { split = 1 } if split >= points.len() - 1 { split = points.len() - 2 }
    for (i, &param) in params.iter().enumerate() {
        let t = find_t(bez, param); let pt = q(bez, t); map.push((pt[0], pt[1], points[i].2));
        let v = subtract(pt, [points[i].0, points[i].1]); let dist = v[0] * v[0] + v[1] * v[1];
        if dist > max_dist { max_dist = dist; if i > 0 && i < points.len() - 1 { split = i } }
    }
    (max_dist, split, map)
}
fn find_t(bez: &[[f64; 2]; 4], param: f64) -> f64 {
    if param <= 0.0 { return 0.0 } if param >= 1.0 { return 1.0 }
    let mut map = vec![0.0]; let mut prev = bez[0]; let mut total = 0.0;
    for i in 1..=10 { let cur = q(bez, i as f64 / 10.0); total += vector_len(subtract(cur, prev)); map.push(total); prev = cur; }
    let map: Vec<f64> = map.iter().map(|x| if total > 0.0 { x / total } else { 0.0 }).collect();
    for i in 1..=10 { if param <= map[i] { let t_min = (i - 1) as f64 / 10.0; let t_max = i as f64 / 10.0; let l_min = map[i - 1]; let l_max = map[i]; return ((param - l_min) / (l_max - l_min)) * (t_max - t_min) + t_min } }
    param
}
// =====================[ SMOOTH STROKE ]=====================
pub fn smooth_stroke(points: &[(f64, f64, f64)]) -> Vec<(f64, f64, f64)> {
    if points.len() < 3 { return points.to_vec() }
    let rw = 1920.0; let rh = 1080.0;
    let scaled: Vec<(f64, f64, f64)> = points.iter().map(|p| (p.0 * rw, p.1 * rh, p.2)).collect();
    let result = fit_curve(&scaled, 10.0);
    result.m.iter().map(|p| (p.0 / rw, p.1 / rh, p.2)).collect()
}
