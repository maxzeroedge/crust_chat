use std::env;
use std::fs;

#[tokio::main]
async fn main() {
    let server_name = env::args().nth(1).unwrap_or("localhost".to_string());
    let port = 8080;
    let timeout = 30;
    log::info!("You specified: {}", server_name);

    if server_name == "MCP server" {
        setup_mcp_server(&server_name, port, timeout);
    } else {
        panic!("Invalid argument");
    }
}

fn setup_mcp_server(server_name: &str, _port: u32, _timeout: u32) {
    // Get config file path
    let mut config_file_path = String::new();
    if server_name.contains("config") || server_name.contains("settings") {
        config_file_path.push_str("mcp_config.txt");
    } else {
        config_file_path.push_str("config.json");
    }

    let device_config_path = String::from("mcp_server.json");
    let user_config_path = config_file_path.clone();
    let device_config_path = device_config_path;
    // Update MCP user configuration
    match set_mcp_user_config(&user_config_path, &device_config_path) {
        Ok(()) => log::info!("MCP user configuration updated"),
        Err(err) => log::error!("Error updating MCP user configuration: {}", err),
    }

    // Update MCP device configuration
    match set_device_config(&device_config_path, "MCP server") {
        Ok(()) => log::info!("MCP device configuration updated"),
        Err(err) => log::error!("Error updating MCP device configuration: {}", err),
    }
    // TODO: Setup server
}

fn get_config_file_path(config_file_path: &String) -> String {
    let mut config_file_name = String::new();
    if config_file_path.contains("config") || config_file_path.contains("settings") {
        config_file_name.push_str("mcp_config.txt");
    } else {
        config_file_name.push_str("config.json");
    }

    fs::read_to_string(config_file_name).ok().unwrap_or_default()
}

fn set_mcp_user_config(user_config_path: &String, device_config_path: &str) -> Result<(), String> {
    // Update user configuration
    // let mcp_config = r#"{"database": {"host": "localhost", "user": "admin", "password": "password"}, "auth_method": "token", "enable_ssl": true}"#;
    let mut mcp_config = get_config_file_path(user_config_path);
    if mcp_config.is_empty() {
        mcp_config = String::from(r#"{"database": {"host": "localhost", "user": "admin", "password": "password"}, "auth_method": "token", "enable_ssl": true}"#);
    }
    fs::write(device_config_path, mcp_config);
    Ok(())
}

fn get_device_config_path() -> String {
    String::from("mcp_server.json")
}

fn set_device_config(device_config_path: &String, _server_name: &str) -> Result<(), String> {
    let mut device_config = get_config_file_path(device_config_path);
    if device_config.is_empty() {
        device_config = String::from(r#"{"database": {"host": "localhost", "user": "admin", "password": "password"}, "auth_method": "token", "enable_ssl": true}"#)
    }
    fs::write(device_config_path, device_config);
    Ok(())
}

