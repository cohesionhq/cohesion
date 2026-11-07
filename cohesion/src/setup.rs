//! `cohesion setup`: registers this client with a setup code, or points it at a local
//! cohesiond, and saves the resulting configuration.

mod host_info;
mod setup_args;

pub use setup_args::SetupArgs;

use std::path::{self, PathBuf};

use cohesion_core::{
    Authentication, Client, LocalIdentity, SetupCode, paths,
    protocol::{ListWorkspacesRequest, Outcome, RegisterRequest},
};

use crate::{
    cli_context::{expect_reply, storage_path},
    cli_error::CliError,
    client_config::{Configuration, Server},
    client_identity::ClientIdentity,
    client_storage::ApplicationStorageError,
    local_connection::LocalConnection,
    remote_connection::RemoteConnection,
};

/// Connects to a local cohesiond with `--local`, else registers using the setup code.
pub async fn run(config_directory: Option<&str>, args: SetupArgs) -> Result<(), CliError> {
    match args.local {
        Some(socket) => run_local(config_directory, socket, args.name).await,
        None => {
            let setup_code = args
                .setup_code
                .expect("clap requires a setup code without --local");
            register(config_directory, &setup_code, args.name).await
        }
    }
}

async fn register(
    config_directory: Option<&str>,
    setup_code: &str,
    name: Option<String>,
) -> Result<(), CliError> {
    let name = name.unwrap_or_else(default_name);
    let setup_code = SetupCode::decode(setup_code).map_err(CliError::InvalidSetupCode)?;
    let storage_path = storage_path(config_directory)?;
    let key = Authentication::generate()
        .map_err(|err| CliError::Storage(ApplicationStorageError::CannotCreateKey(err)))?;
    let pem = key
        .self_signed_certificate_pem(&name)
        .map_err(|err| CliError::Storage(ApplicationStorageError::CannotCreateCertificate(err)))?;

    // Parsed from the PEM we save: signing again would give a different certificate.
    let identity = ClientIdentity::from_pem(pem.as_bytes()).map_err(CliError::Storage)?;
    let connection = RemoteConnection::new(
        &setup_code.host,
        setup_code.port,
        setup_code.fingerprint.clone(),
        identity,
    )
    .map_err(CliError::Setup)?;
    let address = setup_code.address();
    let request = RegisterRequest {
        setup_key: setup_code.key,
        client_name: name,
    };
    let outcome = connection
        .register(request)
        .await
        .map_err(|err| CliError::Unreachable(address, err))?;
    let client: Client = expect_reply(outcome)?;
    let config = Configuration {
        server: Server::Remote {
            host: setup_code.host,
            port: setup_code.port,
            server_fingerprint: setup_code.fingerprint,
        },
        workspace: None,
    };

    ClientIdentity::save_pem(&storage_path, &pem).map_err(CliError::Storage)?;
    config.save(&storage_path).map_err(CliError::Storage)?;
    println!(
        "Registered as {:?} (client {}). Configuration saved to {}.",
        client.client_name,
        client.id,
        storage_path.root().display()
    );
    Ok(())
}

/// Points this client at a cohesiond on this machine. The socket's file
/// permissions stand in for registration; a new local key identifies this
/// client from now on, under `name`.
async fn run_local(
    config_directory: Option<&str>,
    socket: Option<PathBuf>,
    name: Option<String>,
) -> Result<(), CliError> {
    let socket = match socket {
        // Absolute so later commands work from any directory.
        Some(socket) => path::absolute(&socket).unwrap_or(socket),
        None => paths::default_socket_path().ok_or(CliError::NoDefaultSocket)?,
    };
    let storage_path = storage_path(config_directory)?;

    let identity = LocalIdentity::generate(name.unwrap_or_else(default_name));
    // Fail now on a wrong path rather than on every later command.
    let connection = LocalConnection::new(socket.clone(), identity.clone());
    match connection.send(ListWorkspacesRequest.into()).await {
        Ok(Outcome::Ok(_)) => {}
        Ok(Outcome::Error(err)) => return Err(CliError::Server(err)),
        Err(err) => return Err(CliError::Unreachable(socket.display().to_string(), err)),
    }

    let config = Configuration {
        server: Server::Local {
            socket: socket.clone(),
            identity,
        },
        workspace: None,
    };
    config.save(&storage_path).map_err(CliError::Storage)?;
    println!(
        "Connected to cohesiond at {}; configuration saved to {}.",
        socket.display(),
        storage_path.root().display()
    );
    Ok(())
}

/// How a client is listed unless named: this computer's hostname.
pub fn default_name() -> String {
    host_info::hostname().unwrap_or_else(|| "cohesion".into())
}
