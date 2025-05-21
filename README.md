# Lilac 🌸 
> *"The lilac branches are bowed under the weight of the flowers: blooming is hard, and the most important thing is – to bloom.” – Yevgeny Zamyatin*
## Overview
Lilac is a **KISS** AUR helper designed to search, download, and build packages from the Arch User Repository. It's fast, lightweight, and doesn't try to reinvent the wheel.
## Installation
1. Clone the repository:
   ```bash
   (Main Repository)
   git clone https://git.cyno.space/ryan/lilac.git

   (Mirror)
   git clone https://github.com/snoooooooope/lilac.git

   cd lilac
   ```
2. Install:
   ```bash
   cargo install --path .
   ```
## Usage
```bash
# Search for packages
lilac search stuxnet
# Install a package
lilac install stuxnet
# Remove a package
lilac remove stuxnet
# Get package info
lilac info stuxnet
# Get package info (including deps)
lilac info stuxnet --deps
```



## Issues
- Report issues: [Cyno Issues](https://git.cyno.space/issues) | [GitHub Issues](https://github.com/snoooooooope/lilac/issues)
## Contributing
Contributions are welcome from everyone. Here's how you can help:
   ## Requirements
   - Git
   - Jujutsu
   - Rust   
### JJ
1. **Clone the repository**:
   ```bash
   jj git clone https://git.cyno.space/ryan/lilac.git
   ```
2. **Create a feature**:
   ```bash
   jj new feature
   ```
3. **Commit your changes**:
   ```bash
   jj desc -m 'broke everything'
   ```
4. **Push your bookmark**:
   ```bash
   jj git push --change @-
   ```
## License
Lilac is MIT licensed.
