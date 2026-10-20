# Contact Injector

Contact injector was born out of the need so allow our customers phone systems to access their carddav contacts.

Some phone systems need additional software that can cost monthly fees, which in my personal opinion is outrageous.
I wouldn't be happy about a fixed payment, but I could at least understand it.

And others don't support carddav at all.

So here we are - contact injector is not only free, but licensed under the MIT license,
eeady to be expanded or used as a crate by others.

If you encounter any issues, you're welcome to open an issue on GitHub.

## Supported Sources:

- Carddav address books

## Supported Destinations:

- Starface Phone Systems
  (Please create a backup of your phone systems contacts before deploying)

## Installation

The recommended way to install contact injector is using docker.

You can use the [docker-compose file of this repository](./docker-compose.yml) as a baseline.

## Development

The workspace contains the web application and scheduler (`contact_injector`),
contact models and adapters (`contact_protocols`), and DAV transport and parsers
(`dav-client`). Run `cargo test --workspace --locked` and
`cargo clippy --workspace --all-targets --locked` to check changes.

Adapters use module-qualified names: `carddav::Source`, `starface::Destination`,
and `caldav_birthdays::Destination`, with `AccessData` and `Error` in each module.
These replace the former protocol-prefixed type names. Stored access-data JSON,
database schemas, and application routes retain their existing formats.

Job delays must be between 1 and 525600 minutes. The private `job::delay::Delay`
type enforces this at form and database boundaries (`smart-constructor-newtype`).
Workers receive commands through channels and publish status with watch channels;
deleting a job waits for an active export to finish before removing its record.

Starface exports construct a nonempty converted batch before reading or changing
destination contacts. Failed uploads stop the export before stale-contact deletion,
and HTTP deletion failures are reported as sync failures.
