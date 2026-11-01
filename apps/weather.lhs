# weather — OpenWeatherMap CLI (Python weather-app shape, without Flask/Tk)
#
# Needs: curl on PATH, and OPENWEATHER_API_KEY in the environment.
#
#   export OPENWEATHER_API_KEY=your_key
#   CITY=Orlando ./target/debug/lhsc run apps/weather.lhs
#   ./target/debug/lhsc build apps/weather.lhs -o weather
#   ./weather London

fn show_weather(city: str, key: str) {
    let url = "http://api.openweathermap.org/data/2.5/weather?q=" + city + "&appid=" + key
    match http_get(url) {
        Ok(body) => {
            print("city: " + city)
            match json_get(body, "name") {
                Some(n) => print("name: " + n),
                None => print("name: (missing)"),
            }
            match json_get(body, "sys.country") {
                Some(c) => print("country: " + c),
                None => print("country: (missing)"),
            }
            match json_get(body, "coord.lon") {
                Some(lon) => {
                    match json_get(body, "coord.lat") {
                        Some(lat) => print("coord: " + lon + " , " + lat),
                        None => print("coord: " + lon),
                    }
                },
                None => print("coord: (missing)"),
            }
            match json_get(body, "main.temp") {
                Some(t) => print("temp: " + t + " K"),
                None => print("temp: (missing)"),
            }
            match json_get(body, "main.pressure") {
                Some(p) => print("pressure: " + p),
                None => print("pressure: (missing)"),
            }
            match json_get(body, "main.humidity") {
                Some(h) => print("humidity: " + h),
                None => print("humidity: (missing)"),
            }
            match json_get(body, "weather.0.description") {
                Some(d) => print("description: " + d),
                None => print("description: (missing)"),
            }
        },
        Err(e) => print(e),
    }
}

fn run_city(city: str) {
    match getenv("OPENWEATHER_API_KEY") {
        Some(key) => show_weather(city, key),
        None => {
            print("Set OPENWEATHER_API_KEY, then run again.")
            print("Example: export OPENWEATHER_API_KEY=...")
            print("City: set CITY=Name or pass Name to the built binary.")
        },
    }
}

fn main() {
    match getenv("CITY") {
        Some(c) => {
            run_city(c)
            return
        },
        None => {},
    }
    # Standalone binary: ./weather London  → arg(1)
    if argc() > 1 {
        match arg(1) {
            Some(a) => {
                if a == "run" {
                    run_city("mathura")
                    return
                }
                if a == "--jit" {
                    run_city("mathura")
                    return
                }
                run_city(a)
                return
            },
            None => {},
        }
    }
    run_city("mathura")
}
