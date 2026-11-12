import os


def get_directory_tree(directory, ignore_spec=None, max_depth=2, depth=0, project_root=None, prefix="",
                       only_dirs=False):
    """Return a tree-style directory listing, honoring .gitignore rules."""
    if max_depth is not None and depth >= max_depth:
        return ""

    if project_root is None:
        project_root = os.path.abspath(directory)

    entries = sorted(os.listdir(directory))
    entries = [e for e in entries if not e.startswith(".")]

    tree_lines = []

    for index, entry in enumerate(entries):
        path = os.path.join(directory, entry)
        relative_path = os.path.relpath(path, start=project_root)

        if only_dirs and not os.path.isdir(path):
            continue

        if os.path.isdir(path):
            relative_path += "/"

        if ignore_spec and ignore_spec.match_file(relative_path):
            continue

        is_last = (index == len(entries) - 1)
        connector = "└── " if is_last else "├── "
        tree_lines.append(prefix + connector + entry)

        if os.path.isdir(path):
            new_prefix = prefix + ("    " if is_last else "│   ")
            sub_tree = get_directory_tree(path, ignore_spec, max_depth, depth + 1, project_root, new_prefix,
                                          only_dirs=only_dirs)
            if sub_tree:
                tree_lines.extend(sub_tree.split("\n"))

    return "\n".join(tree_lines)
