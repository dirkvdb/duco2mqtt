# Project goal
Query the http api of a home ventilation system and expose the data to an MQTT broker

# Project layout
- `src`: Rust source code.
- `src/brigde.rs`: The main application loop that polls the http api and owns the MQTT connection.
- `src/ducoapi.rs`: Wrapper around the home ventilation system http api.
- `src/mqtt.rs`: MQTT client logic.

# Building the project
To build the projec, you can use the following command:
```bash
just build
```

To run the tests, use:
```bash
just test
```

# Guidelines
- After making changes to the project, verify that the build and tests are still passing
- Only add comments for complex logic that is not immediately clear
- Do not crate summary markdown files with the things that have been changed
