import { browser } from '$app/environment';

type Language = 'en' | 'zh';
type Theme = 'light' | 'dark' | 'system';

const translations: Record<Language, Record<string, string>> = {
  en: {
    'nav.dashboard': 'Dashboard',
    'nav.queue': 'Review Queue',
    'nav.browser': 'File Browser',
    'nav.rules': 'Rules',
    'nav.audit': 'Audit',
    'nav.settings': 'Settings',
    'status.active': 'Active',
    'status.paused': 'Paused',
    'status.watchStatus': 'Watch Status',
    'theme.title': 'Theme',
    'theme.light': 'Light',
    'theme.dark': 'Dark',
    'theme.system': 'System',
    'lang.title': 'Language',
    'lang.en': 'English',
    'lang.zh': '简体中文',
    'dashboard.welcome':
      'Welcome to ShelfLife. Select an option from the sidebar to manage or browse your files.',
    'dashboard.triageNeeded': 'Triage Needed',
    'dashboard.triageDesc': 'Files are stale or decaying',
    'dashboard.healthyFiles': 'Healthy Files',
    'dashboard.healthyDesc': 'Files are fresh or permanent',
    'dashboard.ignoredFiles': 'Ignored Files',
    'dashboard.ignoredDesc': 'Files excluded from active rules',
    'dashboard.subtitle': 'Manage and triage files before they expire.',
    'dashboard.search': 'Search files by name...',
    'dashboard.noFiles': 'No files found in the queue.',
    'dashboard.clearSelection': 'Clear Selection',
    'dashboard.errorLoading': 'Error loading queue',
    'dashboard.loadingQueue': 'Loading queue...',
    'dashboard.noFilesDesc': 'No files require action at this time. Good job!',
    'dashboard.loadMore': 'Load More ({count} remaining)',
    'status.review': 'Review',
    'status.tracked': 'Tracked',
    'status.recoverableSize': 'Recoverable Size',
    'tab.all': 'All',
    'tab.fresh': 'Fresh',
    'tab.stale': 'Stale',
    'tab.decaying': 'Decaying',
    'tab.pinned': 'Pinned',
    'tab.ignored': 'Ignored',
    'file.firstSeen': 'First Seen',
    'file.snooze': 'Snooze',
    'file.pin': 'Pin Permanent',
    'file.ignore': 'Ignore',
    'file.trash': 'Trash Now',
    'file.safeFolder': 'Safe Folder',
    'file.errorExplanation': 'Could not load explanation.',
    'file.errorAction': 'Action failed.',
    'file.actionMove': 'Move',
    'file.actionLabel': 'Action',
    'file.confirmMsg':
      '{action} will be recorded in the audit log for {name}. Undo availability depends on the action and file state.',
    'file.errorOpenLocation': 'Could not open file location.',
    'file.openFolder': 'Open Folder',
    'file.moveTitle': 'Move Destination',
    'file.movePlaceholder': 'Absolute folder path...',
    'file.snoozeTitle': 'Snooze Expiry',
    'file.snoozeCustom': 'Custom',
    'file.day': 'day',
    'file.days': 'days',
    'file.noRule': 'No rule',
    'file.noRuleMatched': 'No rule matched',
    'file.protected': 'Protected',
    'rules.title': 'Automation Rules',
    'rules.subtitle': 'Define automatic cleanup actions for decaying files.',
    'rules.newRule': 'New Rule',
    'rules.noRules': 'No automation rules configured.',
    'rules.priority': 'Priority',
    'rules.mode': 'Mode',
    'rules.modePreviewOnly': 'Preview Only',
    'rules.modeAskFirst': 'Ask First',
    'rules.modeAutomatic': 'Automatic',
    'rules.action': 'Action',
    'rules.enabled': 'Enabled',
    'rules.disabled': 'Disabled',
    'rules.testRule': 'Test Rule',
    'rules.testResults': 'Test Results',
    'rules.refresh': 'Refresh',
    'rules.editRule': 'Edit Rule: {name}',
    'rules.errorDelete': 'Could not delete rule.',
    'rules.errorUpdateStatus': 'Could not update rule status.',
    'rules.errorTest': 'Could not test rule.',
    'rules.loading': 'Loading automation rules...',
    'rules.noRulesDesc':
      'Create a rule to automate trashing, moving, or ignoring files in your watch targets.',
    'rules.watchTarget': 'Watch target: {path}',
    'rules.ttlDays': 'TTL: {days} days',
    'rules.edit': 'Edit',
    'rules.testing': 'Testing...',
    'rules.clearResults': 'Clear Results',
    'rules.errorSelectFolder': 'Could not select folder.',
    'rules.errorSaveRule': 'Could not save rule.',
    'rules.generalSettings': 'General Settings',
    'rules.ruleName': 'Rule Name',
    'rules.ruleNamePlaceholder': 'e.g. Clean Temporary Downloads',
    'rules.watchTargetPath': 'Watch Target Path',
    'rules.ttlDaysLabel': 'TTL (Days)',
    'rules.matchConditions': 'Match Conditions',
    'rules.extensions': 'Extensions',
    'rules.extensionsPlaceholder': 'pdf, zip, png (comma-sep)',
    'rules.filenameGlobs': 'Filename Globs',
    'rules.filenameGlobsPlaceholder': '*.tmp, temp_*',
    'rules.filenameRegexes': 'Filename Regexes',
    'rules.filenameRegexesPlaceholder': '(?i)copy',
    'rules.sourceDomains': 'Source Domains',
    'rules.sourceDomainsPlaceholder': 'google.com, github.com',
    'rules.fileSizeCriteria': 'File Size Criteria',
    'rules.anySize': 'Any Size',
    'rules.lessThan': 'Less Than',
    'rules.greaterThan': 'Greater Than',
    'rules.between': 'Between',
    'rules.minSizeMb': 'Min Size (MB)',
    'rules.maxSizeMb': 'Max Size (MB)',
    'rules.actionIgnoreLabel': 'Ignore (Keep in place)',
    'rules.actionMoveLabel': 'Move out of Watch Folder',
    'rules.destinationPath': 'Destination Folder',
    'rules.renameTemplate': 'Optional Rename Template',
    'rules.saveRule': 'Save Rule',
    'rules.testResultsCount': 'Dry Run Match Results ({count} files matched)',
    'audit.title': 'Audit Log & History',
    'audit.subtitle': 'Track file management history and undo actions.',
    'audit.noLogs': 'No audit log entries recorded.',
    'audit.undo': 'Undo',
    'audit.loading': 'Loading audit entries...',
    'audit.noLogsDesc':
      'Actions performed on files will be shown here, allowing you to review or revert them.',
    'audit.errorUndo': 'Undo failed',
    'audit.source': 'Source: {path}',
    'audit.destLabel': 'Dest: {path}',
    'audit.size': 'Size: {size}',
    'audit.rule': 'Rule: {name}',
    'audit.undoing': 'Undoing...',
    'audit.undone': 'Undone',
    'settings.title': 'Settings',
    'settings.subtitle': 'Configure watch targets and application behaviors.',
    'settings.watchTargets': 'Watch Targets',
    'settings.addNewTarget': 'Add Watch Target',
    'settings.noTargets': 'No watch targets defined.',
    'settings.path': 'Folder Path',
    'settings.savedShort': 'Saved',
    'settings.general': 'General Settings',
    'settings.safeFolder': 'Safe Folder Path',
    'settings.notifications': 'Enable Notifications',
    'settings.notificationsDesc':
      'Receive desktop alerts before files expire or when automatic cleanup runs.',
    'settings.startAtLogin': 'Start ShelfLife on boot',
    'settings.startAtLoginDesc': 'Automatically run ShelfLife in the background on system startup.',
    'settings.dropzone': 'Shake-to-dropzone',
    'settings.dropzoneDesc':
      'While dragging files, shake the mouse to show a small dropzone near the cursor.',
    'settings.closeBehavior': 'When closing the window',
    'settings.closeAsk': 'Ask every time',
    'settings.closeHideToTray': 'Keep running in tray',
    'settings.closeQuit': 'Quit app',
    'settings.removeConfirmTitle': 'Remove Watch Target',
    'settings.removeConfirmText': 'Are you sure you want to remove this watch target?',
    'settings.overlapConfirmTitle': 'Overlapping Watch Target',
    'settings.overlapConfirmText':
      'This folder overlaps existing watch targets. Use the new folder to remove the overlapping targets and add this one, or keep your current targets.\n\nNew folder:\n{path}\n\nOverlapping targets:\n{paths}',
    'settings.overlapUseNew': 'Use New Folder',
    'settings.overlapKeepExisting': 'Keep Existing',
    'settings.remove': 'Remove',
    'rules.deleteConfirmTitle': 'Delete Automation Rule',
    'rules.deleteConfirmText': 'Are you sure you want to delete this rule?',
    'rules.delete': 'Delete',
    'dialog.confirmTitle': 'Confirm Action',
    'dialog.confirmText': 'Are you sure you want to perform this action?',
    'dialog.yes': 'Yes',
    'dialog.no': 'No',
    'dialog.cancel': 'Cancel',
    'closeDialog.title': 'Close ShelfLife?',
    'closeDialog.message': 'ShelfLife can keep watching files from the tray, or quit now.',
    'closeDialog.remember': 'Remember my choice',
    'closeDialog.keepRunning': 'Keep running in tray',
    'closeDialog.quit': 'Quit ShelfLife',
    'closeDialog.error': 'Could not apply close behavior.',
    'settings.browse': 'Browse...',
    'settings.defaultTtlDays': 'Default Expiry (TTL Days)',
    'settings.staleAge': 'Stale Age (Days)',
    'settings.decayBuffer': 'Decay Buffer (Hours)',
    'settings.decayTimeline': 'File Decay & Expiry Timeline',
    'settings.decayTimelineDesc':
      'Adjust the handles to set how long files remain Fresh, when they become Stale, and when the warning Decay Buffer begins prior to Expiry.',
    'settings.freshZone': 'Fresh Zone',
    'settings.freshZoneDesc': 'Recently active files.',
    'settings.staleZone': 'Stale Zone',
    'settings.staleZoneDesc': 'Inactive files; review suggested.',
    'settings.decayingZone': 'Decaying Zone',
    'settings.decayingZoneDesc': 'Approaching cleanup deadline.',
    'settings.expiredZone': 'Expired Zone',
    'settings.expiredZoneDesc': 'Cleanup action triggered.',
    'settings.dayUnit': 'day',
    'settings.daysUnit': 'days',
    'settings.saving': 'Saving Preferences...',
    'settings.enabled': 'Enabled',
    'settings.disabled': 'Disabled',
    'settings.recursiveLabel': 'Recursive',
    'settings.topLevel': 'Top Level',
    'settings.errorDuplicate': 'Folder is already a watch target.',
    'settings.errorSafeFolderOverlap':
      'Safe folder and watch targets cannot overlap. Choose separate folders.',
    'settings.errorUpdateTargets': 'Could not update watch targets.',
    'settings.errorSavePrefs': 'Could not save preferences.',
    'settings.errorUpdateTarget': 'Could not update watch target.',
    'settings.errorRemoveTarget': 'Could not remove watch target.',
    'settings.errorSelectFolder': 'Could not select folder.',
    'settings.reconcileScan': 'Re-scan Now',
    'settings.reconcileScanning': 'Scanning...',
    'settings.errorReconcileScan': 'Manual scan failed.',
    'browser.folders': 'Folders',
    'browser.fileCountSingular': '{count} file',
    'browser.fileCountPlural': '{count} files',
    'browser.loadMoreFolders': 'Load More Folders ({count} remaining)',
    'browser.files': 'Files',
    'browser.selectReviewable': 'Select Reviewable',
    'browser.selectAll': 'Select All',
    'browser.loadMoreFiles': 'Load More Files ({count} remaining)',
    'browser.emptyFolder': 'Empty Folder',
    'browser.emptyFolderDesc': 'This directory does not contain any tracked files or folders.',
    'browser.applyAction': 'Apply Action',
    'browser.bulkConfirmMsg':
      '{action} will be applied to {count} files totaling {size}. Each changed file will create its own audit row.',
    'browser.apply': 'Apply',
    'browser.errorBulkAction': 'Bulk action failed.',
    'browser.bulkSummary': '{action}: {succeeded} succeeded, {failed} failed',
    'dropzone.title': 'Dropzone',
    'dropzone.titleWindow': 'ShelfLife Dropzone',
    'dropzone.subtitle.reading': 'Reading dropped files',
    'dropzone.subtitle.prompt': 'Drop files here',
    'dropzone.subtitle.files': '{count} files, {size}',
    'dropzone.subtitle.file': '{count} file, {size}',
    'dropzone.close': 'Close dropzone',
    'dropzone.previewFailed': 'Dropzone preview failed.',
    'dropzone.actionFailed': 'Dropzone action failed.',
    'dropzone.ruleFailed': 'Rule action failed.',
    'dropzone.watchTargets': 'Watch targets',
    'dropzone.ruleGroups': 'Rule groups',
    'dropzone.badge.preview': '{count} preview',
    'dropzone.badge.unmatched': '{count} unmatched',
    'dropzone.badge.failed': '{count} failed',
    'dropzone.resultSummary': '{completed} completed, {failed} failed',
  },
  zh: {
    'nav.dashboard': '仪表板',
    'nav.queue': '清理队列',
    'nav.browser': '文件浏览器',
    'nav.rules': '自动化规则',
    'nav.audit': '审计日志',
    'nav.settings': '设置',
    'status.active': '监控中',
    'status.paused': '已暂停',
    'status.watchStatus': '监控状态',
    'theme.title': '主题',
    'theme.light': '浅色模式',
    'theme.dark': '深色模式',
    'theme.system': '系统默认',
    'lang.title': '语言',
    'lang.en': 'English',
    'lang.zh': '简体中文',
    'dashboard.welcome': '欢迎使用 ShelfLife。从侧边栏选择一个选项来管理或浏览您的文件。',
    'dashboard.triageNeeded': '需要清理',
    'dashboard.triageDesc': '文件已陈旧或在衰退中',
    'dashboard.healthyFiles': '健康文件',
    'dashboard.healthyDesc': '文件新鲜或永久保存',
    'dashboard.ignoredFiles': '已忽略文件',
    'dashboard.ignoredDesc': '未被自动化规则处理的文件',
    'dashboard.subtitle': '在文件过期前对其进行管理和归档。',
    'dashboard.search': '按文件名搜索...',
    'dashboard.noFiles': '队列中未发现文件。',
    'dashboard.clearSelection': '清除选择',
    'dashboard.errorLoading': '加载队列时出错',
    'dashboard.loadingQueue': '正在加载队列...',
    'dashboard.noFilesDesc': '当前没有需要处理的文件。做的不错！',
    'dashboard.loadMore': '加载更多 (还剩 {count} 个)',
    'status.review': '待清理',
    'status.tracked': '已监控',
    'status.recoverableSize': '可回收大小',
    'tab.all': '全部',
    'tab.fresh': '新鲜',
    'tab.stale': '陈旧',
    'tab.decaying': '衰退中',
    'tab.pinned': '已固定',
    'tab.ignored': '已忽略',
    'file.firstSeen': '首次发现',
    'file.snooze': '延期 (Snooze)',
    'file.pin': '永久固定',
    'file.ignore': '忽略',
    'file.trash': '移至回收站',
    'file.safeFolder': '移动至安全夹',
    'file.errorExplanation': '无法加载规则匹配说明。',
    'file.errorAction': '操作失败。',
    'file.actionMove': '移动',
    'file.actionLabel': '操作',
    'file.confirmMsg':
      '对文件“{name}”的操作“{action}”将被记录在审计日志中。可撤销状态取决于该操作和文件状态。',
    'file.errorOpenLocation': '无法打开文件所在位置。',
    'file.openFolder': '打开文件夹',
    'file.moveTitle': '移动目标路径',
    'file.movePlaceholder': '输入绝对文件夹路径...',
    'file.snoozeTitle': '延期保留时间',
    'file.snoozeCustom': '自定义',
    'file.day': '天',
    'file.days': '天',
    'file.noRule': '无匹配规则',
    'file.noRuleMatched': '无匹配规则',
    'file.protected': '受保护',
    'rules.title': '自动化规则',
    'rules.subtitle': '定义针对过期衰退文件的自动清理动作。',
    'rules.newRule': '新建规则',
    'rules.noRules': '尚未配置自动化规则。',
    'rules.priority': '优先级',
    'rules.mode': '模式',
    'rules.modePreviewOnly': '仅显示预览',
    'rules.modeAskFirst': '先询问我',
    'rules.modeAutomatic': '自动执行',
    'rules.action': '执行动作',
    'rules.enabled': '已启用',
    'rules.disabled': '已禁用',
    'rules.testRule': '测试规则',
    'rules.testResults': '测试结果',
    'rules.refresh': '刷新',
    'rules.editRule': '编辑规则: {name}',
    'rules.errorDelete': '无法删除规则。',
    'rules.errorUpdateStatus': '无法更新规则状态。',
    'rules.errorTest': '无法测试规则。',
    'rules.loading': '正在加载自动化规则...',
    'rules.noRulesDesc': '创建规则以自动移动、忽略监控目标中的文件或将其移至回收站。',
    'rules.watchTarget': '监控目标: {path}',
    'rules.ttlDays': '过期时间: {days} 天',
    'rules.edit': '编辑',
    'rules.testing': '测试中...',
    'rules.clearResults': '清除结果',
    'rules.errorSelectFolder': '无法选择文件夹。',
    'rules.errorSaveRule': '无法保存规则。',
    'rules.generalSettings': '通用设置',
    'rules.ruleName': '规则名称',
    'rules.ruleNamePlaceholder': '例如: 清理临时下载文件',
    'rules.watchTargetPath': '监控目标路径',
    'rules.ttlDaysLabel': '过期时长 (天)',
    'rules.matchConditions': '匹配条件',
    'rules.extensions': '文件扩展名',
    'rules.extensionsPlaceholder': '例如: pdf, zip, png (逗号分隔)',
    'rules.filenameGlobs': '文件名 Glob 匹配',
    'rules.filenameGlobsPlaceholder': '例如: *.tmp, temp_* (逗号分隔)',
    'rules.filenameRegexes': '文件名正则表达式',
    'rules.filenameRegexesPlaceholder': '例如: (?i)copy',
    'rules.sourceDomains': '来源域名',
    'rules.sourceDomainsPlaceholder': '例如: google.com, github.com (逗号分隔)',
    'rules.fileSizeCriteria': '文件大小匹配',
    'rules.anySize': '任意大小',
    'rules.lessThan': '小于',
    'rules.greaterThan': '大于',
    'rules.between': '介于',
    'rules.minSizeMb': '最小大小 (MB)',
    'rules.maxSizeMb': '最大大小 (MB)',
    'rules.actionIgnoreLabel': '忽略 (保留在原处)',
    'rules.actionMoveLabel': '移出监控文件夹',
    'rules.destinationPath': '目标文件夹',
    'rules.renameTemplate': '可选重命名模板',
    'rules.saveRule': '保存规则',
    'rules.testResultsCount': '空运行匹配结果 ({count} 个文件匹配)',
    'audit.title': '审计日志与历史',
    'audit.subtitle': '跟踪所有文件操作历史并执行撤销 (Undo)。',
    'audit.noLogs': '未记录任何审计日志。',
    'audit.undo': '撤销操作',
    'audit.loading': '正在加载审计日志...',
    'audit.noLogsDesc': '在此处显示对文件执行的操作，您可以查看或撤销它们。',
    'audit.errorUndo': '撤销失败',
    'audit.source': '源路径: {path}',
    'audit.destLabel': '目标路径: {path}',
    'audit.size': '大小: {size}',
    'audit.rule': '规则: {name}',
    'audit.undoing': '正在撤销...',
    'audit.undone': '已撤销',
    'settings.title': '应用设置',
    'settings.subtitle': 'Configure watch targets and application behaviors.',
    'settings.watchTargets': '监控目标文件夹',
    'settings.addNewTarget': '添加监控目标',
    'settings.noTargets': '未配置任何监控目标。',
    'settings.path': '文件夹路径',
    'settings.savedShort': '已保存',
    'settings.general': '通用设置',
    'settings.safeFolder': '安全文件夹路径',
    'settings.notifications': '启用桌面通知',
    'settings.notificationsDesc': '在文件即将过期或自动执行清理动作时，接收桌面通知提示。',
    'settings.startAtLogin': '开机自启动',
    'settings.startAtLoginDesc': '在开机登录系统时自动运行 ShelfLife，以保持后台静默监控。',
    'settings.dropzone': '摇动鼠标显示投放区',
    'settings.dropzoneDesc': '拖动文件时摇动鼠标，在光标附近显示小型投放区。',
    'settings.closeBehavior': '关闭窗口时',
    'settings.closeAsk': '每次询问',
    'settings.closeHideToTray': '在托盘中继续运行',
    'settings.closeQuit': '退出应用',
    'settings.removeConfirmTitle': '移除监控目标',
    'settings.removeConfirmText': '您确定要移除该监控目标文件夹吗？',
    'settings.overlapConfirmTitle': '监控目标重叠',
    'settings.overlapConfirmText':
      '该文件夹与现有监控目标重叠。选择使用新文件夹会移除重叠的监控目标并添加此文件夹，也可以保留当前目标。\n\n新文件夹:\n{path}\n\n重叠目标:\n{paths}',
    'settings.overlapUseNew': '使用新文件夹',
    'settings.overlapKeepExisting': '保留现有目标',
    'settings.remove': '移除',
    'rules.deleteConfirmTitle': '删除自动化规则',
    'rules.deleteConfirmText': '您确定要删除该自动化规则吗？',
    'rules.delete': '删除',
    'dialog.confirmTitle': '确认操作',
    'dialog.confirmText': '您确定要执行该操作吗？',
    'dialog.yes': '确定',
    'dialog.no': '取消',
    'dialog.cancel': '取消',
    'closeDialog.title': '关闭 ShelfLife？',
    'closeDialog.message': 'ShelfLife 可以在托盘中继续监控文件，或者现在退出。',
    'closeDialog.remember': '记住我的选择',
    'closeDialog.keepRunning': '在托盘中继续运行',
    'closeDialog.quit': '退出 ShelfLife',
    'closeDialog.error': '无法应用关闭行为。',
    'settings.browse': '浏览...',
    'settings.defaultTtlDays': '默认过期时间 (天)',
    'settings.staleAge': '陈旧时长 (天)',
    'settings.decayBuffer': '衰退缓冲时间 (小时)',
    'settings.decayTimeline': '文件衰退与过期时间线',
    'settings.decayTimelineDesc':
      '调整滑块以设置文件保持新鲜的时间、何时变陈旧，以及过期前多久开始提示衰退。',
    'settings.freshZone': '新鲜期',
    'settings.freshZoneDesc': '最近活跃或被使用的文件。',
    'settings.staleZone': '陈旧期',
    'settings.staleZoneDesc': '闲置未被访问的文件，建议在此阶段清理。',
    'settings.decayingZone': '衰退期',
    'settings.decayingZoneDesc': '临近最后的过期清理截止日期。',
    'settings.expiredZone': '已过期',
    'settings.expiredZoneDesc': '超过此天数后，将触发自动化清理动作。',
    'settings.dayUnit': '天',
    'settings.daysUnit': '天',
    'settings.saving': '正在保存首选项...',
    'settings.enabled': '已启用',
    'settings.disabled': '已禁用',
    'settings.recursiveLabel': '递归监控',
    'settings.topLevel': '仅限顶层',
    'settings.errorDuplicate': '该文件夹已在监控目标中。',
    'settings.errorSafeFolderOverlap': '安全文件夹和监控目标不能重叠。请选择不同的文件夹。',
    'settings.errorUpdateTargets': '无法更新监控目标。',
    'settings.errorSavePrefs': '无法保存首选项。',
    'settings.errorUpdateTarget': '无法更新监控目标文件夹。',
    'settings.errorRemoveTarget': '无法移除监控目标。',
    'settings.errorSelectFolder': '无法选择文件夹。',
    'settings.reconcileScan': '立即重新扫描',
    'settings.reconcileScanning': '正在扫描...',
    'settings.errorReconcileScan': '手动扫描失败。',
    'browser.folders': '文件夹',
    'browser.fileCountSingular': '{count} 个文件',
    'browser.fileCountPlural': '{count} 个文件',
    'browser.loadMoreFolders': '加载更多文件夹 (还剩 {count} 个)',
    'browser.files': '文件',
    'browser.selectReviewable': '选择待清理文件',
    'browser.selectAll': '全选',
    'browser.loadMoreFiles': '加载更多文件 (还剩 {count} 个)',
    'browser.emptyFolder': '空文件夹',
    'browser.emptyFolderDesc': '此目录不包含任何监控的文件或文件夹。',
    'browser.applyAction': '执行操作',
    'browser.bulkConfirmMsg':
      '操作“{action}”将应用到 {count} 个文件 (总大小: {size})。每个变更的文件都将创建一个审计日志记录。',
    'browser.apply': '应用',
    'browser.errorBulkAction': '批量操作失败。',
    'browser.bulkSummary': '操作“{action}”已应用: {succeeded} 个成功，{failed} 个失败。',
    'dropzone.title': '投放区',
    'dropzone.titleWindow': 'ShelfLife 投放区',
    'dropzone.subtitle.reading': '正在读取投放的文件',
    'dropzone.subtitle.prompt': '拖放文件至此处',
    'dropzone.subtitle.files': '{count} 个文件, {size}',
    'dropzone.subtitle.file': '{count} 个文件, {size}',
    'dropzone.close': '关闭投放区',
    'dropzone.previewFailed': '投放区预览失败。',
    'dropzone.actionFailed': '投放区操作失败。',
    'dropzone.ruleFailed': '规则操作失败。',
    'dropzone.watchTargets': '监控目标',
    'dropzone.ruleGroups': '规则组',
    'dropzone.badge.preview': '{count} 个预览',
    'dropzone.badge.unmatched': '{count} 个未匹配',
    'dropzone.badge.failed': '{count} 个失败',
    'dropzone.resultSummary': '{completed} 个完成，{failed} 个失败',
  },
};

class AppState {
  currentLang = $state<Language>('en');
  currentTheme = $state<Theme>('system');

  initialized = false;

  constructor() {
    if (browser) {
      const savedLang = localStorage.getItem('shelflife_lang') as Language;
      if (savedLang === 'en' || savedLang === 'zh') {
        this.currentLang = savedLang;
      }

      const savedTheme = localStorage.getItem('shelflife_theme') as Theme;
      if (savedTheme === 'light' || savedTheme === 'dark' || savedTheme === 'system') {
        this.currentTheme = savedTheme;
      }

      // Sync theme and language choices across windows
      window.addEventListener('storage', (e) => {
        if (e.key === 'shelflife_theme') {
          const newTheme = e.newValue as Theme;
          if (newTheme === 'light' || newTheme === 'dark' || newTheme === 'system') {
            this.currentTheme = newTheme;
          }
        }
        if (e.key === 'shelflife_lang') {
          const newLang = e.newValue as Language;
          if (newLang === 'en' || newLang === 'zh') {
            this.currentLang = newLang;
          }
        }
      });
    }
  }

  init() {
    if (this.initialized || !browser) return;
    this.initialized = true;

    // Sync HTML lang attribute
    $effect(() => {
      document.documentElement.setAttribute('lang', this.currentLang);
    });

    // Handle Theme side effects
    $effect(() => {
      const root = document.documentElement;
      const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');

      const applyTheme = () => {
        const isDark =
          this.currentTheme === 'dark' || (this.currentTheme === 'system' && mediaQuery.matches);

        if (isDark) {
          root.classList.add('dark');
        } else {
          root.classList.remove('dark');
        }
      };

      applyTheme();

      if (this.currentTheme === 'system') {
        mediaQuery.addEventListener('change', applyTheme);
        return () => mediaQuery.removeEventListener('change', applyTheme);
      }
    });
  }

  setLang(lang: Language) {
    this.currentLang = lang;
    if (browser) {
      localStorage.setItem('shelflife_lang', lang);
    }
  }

  setTheme(theme: Theme) {
    this.currentTheme = theme;
    if (browser) {
      localStorage.setItem('shelflife_theme', theme);
    }
  }

  // Translation helper function t
  t(key: string, replacements?: Record<string, string | number>): string {
    const dict = translations[this.currentLang] || translations.en;
    let text = dict[key] || translations.en[key] || key;
    if (replacements) {
      for (const [k, v] of Object.entries(replacements)) {
        text = text.replace(`{${k}}`, String(v));
      }
    }
    return text;
  }
}

export const i18n = new AppState();
