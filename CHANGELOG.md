# Changelog

この file には主な変更を記録する
書式は [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)、版は [Semantic Versioning](https://semver.org/) に従う

## [Unreleased]

### Added

- `read_tree`: binary FBX (版 7100-7700) の node tree を `Document` / `Node` / `Prop` で返す 全ての property の型と、生および zlib の配列を読む
- 合わない file は panic せず offset を添えた `FbxError` で拒否する (範囲外の offset・長さ、null record で閉じない列、宣言と違う長さの配列、256 MiB を超える配列、64 段を超える入れ子)
