use nalgebra::{Vector2, Vector3,};

// The following code is adapted from
// https://www.geometrictools.com/GTE/Mathematics/DistPointTriangle.h
// The internet suggests this is an effecient implementation and I'm too lazy to benchmark it
// myself.
pub(crate) fn triangle_closest_point(
    point: &Vector3<f64>,
    v0: &Vector3<f64>,
    v1: &Vector3<f64>,
    v2: &Vector3<f64>
) -> Vector3<f64>
{
    let diff = v0 - point;
    let edge0 = v1 - v0;
    let edge1 = v2 - v0;

    let a00 = edge0.dot(&edge0);
    let a01 = edge0.dot(&edge1);
    let a11 = edge1.dot(&edge1);

    let b0 = diff.dot(&edge0);
    let b1 = diff.dot(&edge1);

    let f00 = b0;
    let f10 = b0 + a00;
    let f01 = b0 + a01;

    let p = if f00 >= 0.0 {
        if f01 >= 0.0 {
            // (1) p0 = (0,0), p1 = (0,1), H(z) = G(L(z))
            get_min_edge02(a11, b1)
        } else {
            // (2) p0 = (0,t10), p1 = (t01,1-t01),
            // H(z) = (t11 - t10)*G(L(z))
            let p0 = Vector2::new(0.0, f00 / (f00 - f01));
            let tmp = f01 / (f01 - f10);
            let p1 = Vector2::new(tmp, 1.0 - tmp);
            let dt1 = p1[1] - p0[1];
            let h0 = dt1 * (a11 * p0[1] + b1);
            if h0 >= 0.0 {
                get_min_edge02(a11, b1)
            } else {
                let h1 = dt1 * (a01 * p1[0] + a11 * p1[1] + b1);
                if h1 <= 0.0 {
                    get_min_edge12(a01, a11, b1, f10, f01)
                } else {
                    get_min_interior(&p0, h0, &p1, h1)
                }
            }
        }
    } else if f01 <= 0.0 {
        if f10 <= 0.0 {
            // (3) p0 = (1,0), p1 = (0,1), H(z) = G(L(z)) - F(L(z))
            get_min_edge12(a01, a11, b1, f10, f01)
        } else {
            // (4) p0 = (t00,0), p1 = (t01,1-t01), H(z) = t11*G(L(z))
            let p0 = Vector2::new(f00 / (f00 - f10), 0.0);
            let tmp = f01 / (f01 - f10);
            let p1 = Vector2::new(tmp, 1.0 - tmp);
            let h0 = p1[1] * (a01 * p0[0] + b1);
            if h0 >= 0.0 {
                p0  // GetMinEdge01
            } else {
                let h1 = p1[1] * (a01 * p1[0] + a11 * p1[1] + b1);
                if h1 <= 0.0 {
                    get_min_edge12(a01, a11, b1, f10, f01)
                } else {
                    get_min_interior(&p0, h0, &p1, h1)
                }
            }
        }
    } else if f10 <= 0.0 {
        // (5) p0 = (0,t10), p1 = (t01,1-t01),
        // H(z) = (t11 - t10)*G(L(z))
        let p0 = Vector2::new(0.0,  f00 / (f00 - f01));
        let tmp = f01 / (f01 - f10);
        let p1 = Vector2::new(tmp, 1.0 - tmp);
        let dt1 = p1[1] - p0[1];
        let h0 = dt1 * (a11 * p0[1] + b1);
        if h0 >= 0.0 {
            get_min_edge02(a11, b1)
        } else {
            let h1 = dt1 * (a01 * p1[0] + a11 * p1[1] + b1);
            if h1 <= 0.0 {
                get_min_edge12(a01, a11, b1, f10, f01)
            } else {
                get_min_interior(&p0, h0, &p1, h1)
            }
        }
    } else {
        // (6) p0 = (t00,0), p1 = (0,t11), H(z) = t11*G(L(z))
        let p0 = Vector2::new(f00 / (f00 - f10), 0.0);
        let p1 = Vector2::new(0.0, f00 / (f00 - f01));
        let h0 = p1[1] * (a01 * p0[0] + b1);
        if h0 >= 0.0 {
            p0  // GetMinEdge01
        } else {
            let h1 = p1[1] * (a11 * p1[1] + b1);
            if h1 <= 0.0 {
                get_min_edge02(a11, b1)
            } else {
                get_min_interior(&p0, h0, &p1, h1)
            }
        }
    };


    v0 + p[0] * edge0 + p[1] * edge1
}

pub(crate) fn triangle_distance_sqr(
    point: &Vector3<f64>,
    v0: &Vector3<f64>,
    v1: &Vector3<f64>,
    v2: &Vector3<f64>
) -> f64
{
    let diff = point - triangle_closest_point(point, v0, v1, v2);
    diff.dot(&diff)
}

fn get_min_edge02(a11: f64, b1: f64) -> Vector2<f64> {
    Vector2::new(
        0.0,
        if b1 >= 0.0 {
            0.0
        } else if a11 + b1 <= 0.0 {
            1.0
        } else {
            -b1 / a11
        }
    )
}

fn get_min_edge12(a01: f64, a11: f64, b1: f64, f10: f64, f01: f64) -> Vector2<f64> {
    let h0 = a01 + b1 - f10;

    let snd = if h0 >= 0.0 {
        0.0
    } else {
        let h1 = a11 + b1 - f01;
        if h1 <= 0.0 {
            1.0
        } else {
            h0 / (h0 - h1)
        }
    };
    Vector2::new(1.0 - snd, snd)
}

fn get_min_interior(p0: &Vector2<f64>, h0: f64, p1: &Vector2<f64>, h1: f64) -> Vector2<f64> {
    let z = h0 / (h0 - h1);
    let omz = 1.0 - z;
    Vector2::new(
        omz * p0[0] + z * p1[0],
        omz * p0[1] + z * p1[1],
    )
}

#[test]
fn test_triangle_distance() {
    // Some randomly generated test cases to ensure our implementation works the same as the c++
    // one.
    let round = |f: f64| (f * 1000000.0).round() / 1000000.0;
    let point = [4.2381113609571495, 2.654380745759032, 4.478436557768529].into();
    let v0 = [2.2609034558129397, 0.7363934424590568, 4.659304572410262].into();
    let v1 = [4.871750378385647, 1.5944896964713688, 3.256035491109088].into();
    let v2 = [1.779212205656786, 0.3153905092238507, 4.3033114466181015].into();
    let result = triangle_distance_sqr(&point, &v0, &v1, &v2);
    assert_eq!(round(2.3834888699227306), round(result));

    let point = [3.2996507884558666, 2.888416530961966, 0.055325508435971615].into();
    let v0 = [0.4469603349442086, 3.067410367768883, 4.047282586990228].into();
    let v1 = [2.026616372521026, 3.63393618523506, 3.627165893214766].into();
    let v2 = [2.0951418285736096, 1.7760971249855912, 4.255170120997059].into();
    let result = triangle_distance_sqr(&point, &v0, &v1, &v2);
    assert_eq!(round(14.934459913398761), round(result));

    let point = [3.63001328574124, 0.9742400008433816, 0.46387335118145023].into();
    let v0 = [1.5902550325185771, 2.0151381618856004, 2.3538083350440964].into();
    let v1 = [1.5829566010328917, 2.2573090844329076, 1.4852626251123495].into();
    let v2 = [3.4711046329853827, 2.8842909238485435, 1.1209094996367985].into();
    let result = triangle_distance_sqr(&point, &v0, &v1, &v2);
    assert_eq!(round(3.9993665768144253), round(result));

    let point = [1.357186896450708, 3.639403896176408, 2.774094433133812].into();
    let v0 = [1.6172890396363315, 1.1964919656765431, 4.899777432333721].into();
    let v1 = [3.1343908648935925, 3.310698406970189, 1.995967206937237].into();
    let v2 = [2.7491235792601767, 4.2708245849151005, 2.5801396887048327].into();
    let result = triangle_distance_sqr(&point, &v0, &v1, &v2);
    assert_eq!(round(1.3973700177168618), round(result));

    let point = [4.093430783344875, 1.3740254394134255, 4.434413112423077].into();
    let v0 = [0.5860098196120694, 0.012696049771021012, 1.1539482653141853].into();
    let v1 = [2.4278036299572463, 2.284992994739776, 3.702239903113251].into();
    let v2 = [0.26710102987762385, 1.7806223059369723, 1.9514793956088183].into();
    let result = triangle_distance_sqr(&point, &v0, &v1, &v2);
    assert_eq!(round(4.140253309390491), round(result));

    let point = [0.7550628981335006, 1.044694489765015, 2.3919975501480515].into();
    let v0 = [2.524640179659943, 4.749403785745272, 2.8887203969061934].into();
    let v1 = [1.1538267147503323, 1.0781899077274066, 1.6384947945695294].into();
    let v2 = [3.3930550989939356, 3.200444706485666, 3.328060634863086].into();
    let result = triangle_distance_sqr(&point, &v0, &v1, &v2);
    assert_eq!(round(0.7201797139568188), round(result));

}
