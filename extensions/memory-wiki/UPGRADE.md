# UPGRADE v1 → v2 — 页面组可配置化

## 概要

v2 版本解除了知识库目录的硬编码限制，引入可配置的 `pageGroups`。用户可通过独立配置文件自定义哪些目录被 wiki 插件索引、分组和展示。

## 主要变更

### 🆕 新增能力

| 能力 | 说明 |
|------|------|
| **递归扫描** | `collectMarkdownFiles` 从非递归 `readdir` 改为递归 `walkDir`，嵌套子目录（如 `entities/tech/ai/`）中的 `.md` 文件可被正常索引 |
| **自定义目录分组** | 通过知识库 vault 根目录的 `.wiki-page-groups.json` 配置文件自定义索引目录 |
| **向后兼容** | 不提供配置文件时，行为与 v1 完全一致（默认索引 sources/entities/concepts/syntheses + reports） |

### ❌ 移除

- 移除了 `COMPILE_PAGE_GROUPS` 硬编码常量（`compile.ts`）
- 移除了 `QUERY_DIRS` 硬编码常量（`query.ts`）
- 移除了 `status.ts` 中 `collectVaultCounts` 的硬编码目录列表
- `markdown.ts` 中 `inferWikiPageKind()` 不再依赖路径前缀匹配的硬编码常量

### 🔧 内部重构

| 文件 | 变更 |
|------|------|
| `src/config.ts` | 新增 `WikiPageGroup` 类型、`DEFAULT_PAGE_GROUPS`、`WIKI_PAGE_GROUPS_CONFIG_FILENAME`、`WikiPageGroupSchema`、`loadExtraPageGroupsFromVault()`；`resolveMemoryWikiConfig()` 自动加载 vault 根目录的独立配置 |
| `src/compile.ts` | `COMPILE_PAGE_GROUPS` → `deriveAllPageGroups(config)`；`collectMarkdownFiles` 调 `walkDir` 递归遍历 |
| `src/query.ts` | 移除 `QUERY_DIRS`；`listWikiMarkdownFiles` 和 `readQueryableWikiPages` 接收动态 pageGroupDirs |
| `src/status.ts` | `collectVaultCounts` 接收 `config` 参数，通过 `config.pageGroups` 动态获取目录 |
| `src/markdown.ts` | `inferWikiPageKind()` 新增 `pageGroups` 可选参数 |
| `openclaw.plugin.json` | `configSchema` 新增 `pageGroups` 数组定义 |

## 迁移指南

### 不需要迁移

如果你不需要自定义索引目录，**完全无需操作**。插件在无配置文件时行为与 v1 完全相同。

### 需要自定义目录分组

在知识库 vault 根目录创建 `.wiki-page-groups.json`：

```json
{
  "pageGroups": [
    { "kind": "source", "dir": "sources", "heading": "Sources" },
    { "kind": "entity", "dir": "entities", "heading": "Entities" },
    { "kind": "concept", "dir": "concepts", "heading": "Concepts" },
    { "kind": "synthesis", "dir": "syntheses", "heading": "Syntheses" },
    { "kind": "synthesis", "dir": "views", "heading": "Views" },
    { "kind": "synthesis", "dir": "templates", "heading": "Templates" }
  ]
}
```

**字段说明：**
- `kind` — 映射到四种已有页面类型之一：`entity` / `concept` / `source` / `synthesis`
- `dir` — vault 根目录下的相对路径（如 `sources`、`views`、`templates`；`.` 表示 vault 根目录本身）
- `heading`（可选）— 在 index.md 中使用的标题。不提供则自动按目录名首字母大写生成

**优先级链（追加合并）：**

```
openclaw.json 的 pageGroups → .wiki-page-groups.json → 默认值 (sources/entities/concepts/syntheses)
```

### 老用户注意事项

1. **自定义目录需显式声明。** 如果之前通过软链接或手动方式将文件放入不被索引的目录，v2 会将其纳入索引（因为递归扫描），但页面类型需通过配置文件指定 `kind`。
2. **index.md 会重新生成。** compile 后 index.md 会包含自定义分组对应的链接。已有页面不受影响。
3. **搜索范围同步更新。** 自定义目录中的页面会出现在 `wiki_search` 和 `wiki_get` 的结果中。

## 回滚

如果遇到问题，删除 `.wiki-page-groups.json` 并重启 Gateway 即可恢复 v1 行为。
