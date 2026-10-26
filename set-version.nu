def main [version: string] {
    let parsed_version = (
        $version
        | parse -r '^(?<major>0|[1-9][0-9]*)\.(?<minor>0|[1-9][0-9]*)\.(?<patch>0|[1-9][0-9]*)(?<prerelease>-[0-9A-Za-z.-]+)?(?<build>\+[0-9A-Za-z.-]+)?$'
    )
    if ($parsed_version | is-empty) {
        error make {
            msg: $"バージョン '($version)' は有効な SemVer ではありません。例: 1.2.3"
        }
    }

    let root = $env.FILE_PWD
    let cargo_files = [
        ($root | path join "Cargo.toml")
        ($root | path join "src-tauri" "Cargo.toml")
    ]
    for file in $cargo_files {
        open --raw $file
        | from toml
        | update package.version $version
        | to toml
        | save --force $file
    }

    let tauri_config = $root | path join "src-tauri" "tauri.conf.json"
    open $tauri_config
    | update version $version
    | to json --indent 2
    | save --force $tauri_config

    print $"アプリのバージョンを ($version) に更新しました。"
}
