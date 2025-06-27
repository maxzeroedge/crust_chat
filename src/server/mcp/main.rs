use std::env;
use std::fs;
use std::io;
use serde_json;

fn main() {
    let server_name = env::args().nth(1).unwrap_or("localhost".to_string());
    let port = 8080;
    let timeout = 30;

    server_name = server_name;
    port = port;
    timeout = timeout;
    println!("You specified: {}", server_name);

    if server_name == "MCP server" {
        setup_mcp_server(server_name, port, timeout);
    } else {
        panic!("Invalid argument");
    }
}

fn setup_mcp_server(server_name: &str, port: u32, timeout: u32) {
    // Get config file path
    let mut config_file_path = String::new();
    if server_name.contains("config") || server_name.contains("settings") {
        config_file_path.push_str("mcp_config.txt");
    } else {
        config_file_path.push_str("config.json");
    }

    let mut device_config_path = String::from("mcp_server.json");
    let mut user_config_path = config_file_path.clone();
    let device_config_path = device_config_path;
    // Update MCP user configuration
    match set_mcp_user_config(user_config_path, &device_config_path) {
        Ok(() => println!("MCP user configuration updated"),
        Err(err) => eprintln!("Error updating MCP user configuration: {}", err),
    }

    // Update MCP device configuration
    let mut mcp_config = serde_json::to_string(&{"database": {"host": "localhost", "user": "admin", "password": "password"}, "auth_method": "token", "enable_ssl": true}).unwrap();
    match set_device_config(device_config_path, "MCP server") {
        Ok(()) => println!("MCP device configuration updated"),
        Err(err) => eprintln!("Error updating MCP device configuration: {}", err),
    }
}

fn get_config_file_path(config_file_path: &String) -> Result<String, String> {
    let mut config_file_name = String::new();
    if config_file_path.contains("config") || config_file_path.contains("settings") {
        config_file_name.push_str("mcp_config.txt");
    } else {
        config_file_name.push_str("config.json");
    }

    fs::read_to_string(config_file_name)
}

fn set_mcp_user_config(user_config_path: &String, device_config_path: &str) -> Result<(), String> {
    // Update user configuration
    let mut mcp_config = serde_json::to_string(&{"database": {"host": "localhost", "user": "admin", "password": "password"}, "auth_method": "token", "enable_ssl": true})?;
    fs::write(device_config_path, mcp_config)?;
    Ok(())
}

fn get_device_config_path() -> String {
    "mcp_server.json"
}

fn set_device_config(device_config_path: &str, server_name: &str) -> Result<(), String> {
    let device_config = serde_json::to_string(&{"database": {"host": "localhost", "user": "admin", "password": "password"}, "auth_method": "token", "enable_ssl": true})?;
    fs::write(device_config_path, device_config)?;
    Ok(())
}

