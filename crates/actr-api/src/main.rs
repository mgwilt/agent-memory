use actr_api::route_manifest;

fn main() {
    for route in route_manifest() {
        println!("{} {} - {}", route.method, route.path, route.purpose);
    }
}
