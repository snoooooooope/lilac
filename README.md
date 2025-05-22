# Lilac 🌸 
> *"The lilac branches are bowed under the weight of the flowers: blooming is hard, and the most important thing is – to bloom.” – Yevgeny Zamyatin*

## Overview
Lilac is a **KISS** AUR helper designed to search, download, and build packages from the Arch User Repository. It's fast, lightweight, and doesn't try to reinvent the wheel.

## Installation
1. Clone the repository:
   ```bash
   git clone https://git.cyno.space/ryan/lilac.git

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
# Update a packag
lilac update stuxnet
# Remove a package
lilac remove stuxnet
# Get package info
lilac info stuxnet
# Get package info (including deps)
lilac info stuxnet --deps
```
---

## Issues
- Report issues: [Cyno Issues](https://git.cyno.space/issues) | [GitHub Issues](https://github.com/snoooooooope/lilac/issues)

## Contributing to This Project

This project uses [**Jujutsu**](https://github.com/jj-vcs/jj) for version control. If you're familiar with Git, you'll find some differences in how we handle commits and history. Our goal is to maintain a clean, atomic, and well-described commit history.

---

### Getting Started

1.  **Install Jujutsu:** Make sure you have Jujutsu installed on your system.
2.  **Clone the Repository:** Clone the project using Jujutsu:
    ```bash
    jj git clone https://github.com/snoooooooope/lilac.git
    ```
---

### Your Workflow

1.  **Keep Up-to-Date:**
    * Regularly update your local repository with the `main` branch before starting new work:
        ```bash
        jj git fetch
        jj rebase -r main
        ```
2.  **Start New Work:**
    * Create a new commit to start your changes. This commit becomes your editable working copy:
        ```bash
        jj new main
        ```
    * *Tip:* If you're fixing an existing commit, you can use `jj edit <commit_id>`. Jujutsu will automatically rebase descendants.
3.  **Make Your Changes:**
    * Edit the code in your working directory. Jujutsu automatically tracks these changes in your current working-copy commit.
4.  **Describe Your Commit:**
    * Once your changes form a single, logical unit, add a clear commit message:
        ```bash
        jj desc -m "changes"
        ```
    * **Commit Message Format:** Start with a topic (e.g., `cli: add new --foo option` or `docs: update quickstart guide`). Be concise and descriptive.
5.  **Create Atomic Commits:**
    * We highly value small, atomic commits. If your work involves multiple distinct changes (e.g., refactoring *then* a new feature), use Jujutsu's tools to split them:
        * `jj split` (interactive splitting)
        * `jj fold` or `jj squash` (to combine commits)
    * Each commit should represent a single, isolated, and logically complete change.
6.  **Include Tests & Docs:**
    * New code should include corresponding tests.
    * Update documentation as needed.
    * **Important:** Tests and documentation belong in the *same commit* as the code they relate to.
7.  **Review Locally:**
    * Always review your changes before sending them for review:
        ```bash
        jj diff
        jj log
        ```      
---

### Submitting for Review (Pull Requests)

2.  **Push to Github**
    * Push your bookmark to GitHub. Jujutsu will force-push, which is normal:
        ```bash
        jj git push --change @-
        ```
3.  **Open a Pull Request (PR):**
    * On GitHub, create a PR from your bookmark to our `main` branch. Use the PR description for any context or discussion.

---

### Addressing Review Comments

**This is a key difference from typical Git workflows:**

1.  **Amend the Original Commit(s):**
    * **DO NOT** create new "fixup" commits on top of your existing ones. Instead, directly modify the commit(s) that need changes.
    * Use `jj edit <commit_id>` to make a specific commit your working-copy commit. Make your changes and then `jj describe` it.
    * Jujutsu will automatically rebase any descendant commits, keeping your history clean.
2.  **Force Push Again:**
    * After amending your commits, push your bookmark to your fork again:
        ```bash
        jj git push --change @-
        ```
    * This updates your existing PR on GitHub with the revised history.

---

### Note

* We **do not** squash-merge PRs, so ensure your local commit history is clean and atomic *before* approval.

---

## License
Lilac is MIT licensed.
