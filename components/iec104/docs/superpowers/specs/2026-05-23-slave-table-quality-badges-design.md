# 子站数据表品质列改为多位徽章

日期:2026-05-23

## 背景

v1.6.0 打通了品质描述词(IV/NT/SB/BL/OV)端到端链路,并把品质显示从单灯升级为多位徽章。但迁移只覆盖了三处:

- 子站**详情面板** `frontend/src/components/ValuePanel.vue`(可编辑开关)
- 主站**数据表** `master-frontend/src/components/DataTable.vue`(紧凑只读徽章)
- 主站**详情面板** `master-frontend/src/components/ValuePanel.vue`(只读徽章)

唯独**子站主数据表** `frontend/src/components/DataPointTable.vue` 漏了,品质列(`:549-551`)仍是 v1.6.0 之前的单点逻辑:

```html
<span v-if="point.quality_iv" class="quality-dot invalid" title="Invalid">IV</span>
<span v-else class="quality-dot ok" title="Good"></span>
```

它只看 `quality_iv`,无视 NT/SB/BL/OV——所以全正常的点一律显示绿点,即使该点带了 NT/SB 等品质位也看不出来。本变更补齐这处,使子站表格与主站表格一致。

## 目标 / 非目标

**目标:**
- 子站数据表品质列改用共享组件 `QualityIndicator`(紧凑、只读),逐位展示置位品质,与主站数据表行为一致。

**非目标:**
- 不在表格行内做品质编辑(编辑仍集中在右侧详情面板的开关)。
- 不改类型 / DTO / 后端:`DataPointInfo` 在 v1.6.0 已带 `quality_ov/bl/sb/nt`。
- 不动其他列或表格交互(值的双击编辑、虚拟滚动、右键删除等)。

## 改动(单文件 `frontend/src/components/DataPointTable.vue`)

1. **品质单元格**(`:549-551`):替换为与主站表格完全一致的写法:
   ```html
   <td class="col-quality">
     <QualityIndicator
       :quality="{ ov: point.quality_ov, bl: point.quality_bl, sb: point.quality_sb, nt: point.quality_nt, iv: point.quality_iv }"
       :show-ov="point.asdu_type.startsWith('M_ME')"
       :show-help="false"
       compact
     />
   </td>
   ```
2. **import**:`import QualityIndicator from '@shared/components/QualityIndicator.vue'`。
3. **CSS**:`.col-quality` 宽度 `40px → 88px`;删除作废的 `.quality-dot`、`.quality-dot.ok`、`.quality-dot.invalid` 三段。

## 行为(与主站表格统一)

- 全正常的点 → 绿色 `OK`(紧凑模式 `compact` 在无置位时显示 OK)。
- 有置位 → 高亮字母徽章(如 `NT`、`IV`),只读不可点。
- OV 徽章仅当 `asdu_type` 以 `M_ME` 开头(测量类)时出现。
- 表格行内不出现 `(?)` 图例(`show-help=false`);释义图例与编辑仍在右侧详情面板。

## 测试

- 现有 `frontend/tests/dataPointTable.spec.ts`:`dp()` helper 在 v1.6.0 已补齐 5 个品质字段,现有过滤/高亮用例不受影响。
- 新增断言:构造一个 `quality_nt=true` 的点,断言其表格行渲染出带 `lit` class 的 `NT` 徽章;一个全正常点渲染出 `OK`。

## 风险

- 列宽变化(40→88px)会挤占表格水平空间;子站表格列不多(IOA/类型/名称/值/品质/时间戳),88px 与主站一致,可接受。
- `QualityIndicator` 每可见行实例化:子站表格同样虚拟滚动,仅渲染可见行,开销与主站表格一致,无新增性能问题。
