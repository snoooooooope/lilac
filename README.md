# Lilac 🌸 



> *"The lilac branches are bowed under the weight of the flowers: blooming is hard, and the most important thing is – to bloom.” – Yevgeny Zamyatin*



## Overview



Lilac is a **KISS** AUR helper designed to search, download, and build packages from the Arch User Repository. It's fast, lightweight, and doesn't try to reinvent the wheel.



## Installation





1. Clone the repository:

   ```bash

   (Main Repository)

   hg clone https://hg.sr.ht/~snoooooooope/lilac

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

- Report issues: [Sourcehut TODO](https://todo.sr.ht/~snoooooooope/lilac) | [GitHub Issues](https://github.com/snoooooooope/lilac/issues)



## Contributing

Contributions are welcome from everyone. Here's how you can help:

### Mercurial (Preferred)

1. **Clone the repository**:

   ```bash

   hg clone https://hg.sr.ht/~snoooooooope/lilac

   ```

2. **Create a bookmark for your feature**:

   ```bash

   hg bookmark feature/your-feature

   ```

3. **Commit your changes**:

   ```bash

   hg commit -m 'Add some feature'

   ```

4. **Push the bookmark to the remote repository**:

   ```bash

   hg push -B feature/your-feature

   ```

5. **Submit patches via email**:

   - Use `hg email` to send patches to the project's [mailing list](https://lists.sr.ht/~snoooooooope/lilac).

   - Example:

     ```bash

     hg email --rev feature/your-feature --to ~snoooooooope/lilac@lists.sr.ht

     ```

   - Include a clear subject prefix like `[PATCH v1]`.



### Git

1. **Clone the repository**:

   ```bash

   git clone https://github.com/snoooooooope/lilac.git

   ```

2. **Create a feature branch**:

   ```bash

   git checkout -b feature/your-feature

   ```

3. **Commit your changes**:

   ```bash

   git commit -am 'Add some feature'

   ```

4. **Push to the branch**:

   ```bash

   git push origin feature/your-feature

   ```

5. **Open a pull request on GitHub**:

   - Navigate to the repository on GitHub.

   - Click "Compare & pull request" for your pushed branch.

   - Include a clear title and description of your changes.



## License

Lilac is MIT licensed.


