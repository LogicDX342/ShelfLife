import { browser } from '$app/environment';

type Language = 'en' | 'zh';
type Theme = 'light' | 'dark' | 'system';

const translations: Record<Language, Record<string, string>> = {
  en: {
    'nav.dashboard': 'Dashboard',
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
    'dashboard.title': 'File Review Queue',
    'dashboard.subtitle': 'Manage and triage files before they expire.',
    'dashboard.search': 'Search files by name...',
    'dashboard.noFiles': 'No files found in the queue.',
    'dashboard.bulkActions': 'Bulk Actions',
    'dashboard.selected': '{count} items selected',
    'dashboard.bulkTrash': 'Trash Selected',
    'dashboard.bulkIgnore': 'Ignore Selected',
    'dashboard.clearSelection': 'Clear Selection',
    'tab.all': 'All',
    'tab.fresh': 'Fresh',
    'tab.stale': 'Stale',
    'tab.decaying': 'Decaying',
    'tab.pinned': 'Pinned',
    'tab.ignored': 'Ignored',
    'file.size': 'Size',
    'file.firstSeen': 'First Seen',
    'file.expiry': 'Expiry',
    'file.origin': 'Origin',
    'file.matchedRules': 'Matched Rules',
    'file.snooze': 'Snooze',
    'file.pin': 'Pin Permanent',
    'file.unpin': 'Unpin File',
    'file.ignore': 'Ignore',
    'file.unignore': 'Stop Ignoring',
    'file.trash': 'Trash Now',
    'file.safeFolder': 'Safe Folder',
    'file.preview': 'Preview',
    'file.rules': 'Rules',
    'file.ruleMatch': 'Rule Match Details',
    'file.noRuleMatch': 'No rule matched this file.',
    'file.details': 'Details',
    'rules.title': 'Automation Rules',
    'rules.subtitle': 'Define automatic cleanup actions for decaying files.',
    'rules.newRule': 'New Rule',
    'rules.noRules': 'No automation rules configured.',
    'rules.priority': 'Priority',
    'rules.mode': 'Mode',
    'rules.action': 'Action',
    'rules.enabled': 'Enabled',
    'rules.disabled': 'Disabled',
    'rules.testRule': 'Test Rule',
    'rules.testResults': 'Test Results',
    'rules.deleteRule': 'Delete Rule',
    'audit.title': 'Audit Log & History',
    'audit.subtitle': 'Track file management history and undo actions.',
    'audit.noLogs': 'No audit log entries recorded.',
    'audit.action': 'Action',
    'audit.file': 'File',
    'audit.dest': 'Destination',
    'audit.undo': 'Undo',
    'settings.title': 'Settings',
    'settings.subtitle': 'Configure watch targets and application behaviors.',
    'settings.watchTargets': 'Watch Targets',
    'settings.addNewTarget': 'Add Watch Target',
    'settings.noTargets': 'No watch targets defined.',
    'settings.path': 'Folder Path',
    'settings.recursive': 'Recursive',
    'settings.defaultTtl': 'Default TTL (seconds)',
    'settings.ignorePatterns': 'Ignore Patterns (comma separated)',
    'settings.save': 'Save Configuration',
    'settings.saved': 'Configuration saved successfully.',
    'settings.general': 'General Settings',
    'settings.protectedPatterns': 'Protected Patterns',
    'settings.safeFolder': 'Safe Folder Path',
    'settings.notifications': 'Enable Notifications',
    'settings.startup': 'Start at Login',
    'dialog.confirmTitle': 'Confirm Action',
    'dialog.confirmText': 'Are you sure you want to perform this action?',
    'dialog.yes': 'Yes',
    'dialog.no': 'No',
  },
  zh: {
    'nav.dashboard': '仪表板',
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
    'dashboard.title': '文件清理队列',
    'dashboard.subtitle': '在文件过期前对其进行管理和归档。',
    'dashboard.search': '按文件名搜索...',
    'dashboard.noFiles': '队列中未发现文件。',
    'dashboard.bulkActions': '批量操作',
    'dashboard.selected': '已选择 {count} 个文件',
    'dashboard.bulkTrash': '批量移至回收站',
    'dashboard.bulkIgnore': '批量忽略',
    'dashboard.clearSelection': '清除选择',
    'tab.all': '全部',
    'tab.fresh': '新鲜',
    'tab.stale': '陈旧',
    'tab.decaying': '衰退中',
    'tab.pinned': '已固定',
    'tab.ignored': '已忽略',
    'file.size': '大小',
    'file.firstSeen': '首次发现',
    'file.expiry': '过期时间',
    'file.origin': '来源凭证',
    'file.matchedRules': '匹配的规则',
    'file.snooze': '延期 (Snooze)',
    'file.pin': '永久固定',
    'file.unpin': '取消固定',
    'file.ignore': '忽略',
    'file.unignore': '取消忽略',
    'file.trash': '移至回收站',
    'file.safeFolder': '移动至安全夹',
    'file.preview': '预览内容',
    'file.rules': '规则匹配',
    'file.ruleMatch': '规则匹配详情',
    'file.noRuleMatch': '此文件未匹配到任何自动化规则。',
    'file.details': '详情信息',
    'rules.title': '自动化规则',
    'rules.subtitle': '定义针对过期衰退文件的自动清理动作。',
    'rules.newRule': '新建规则',
    'rules.noRules': '尚未配置自动化规则。',
    'rules.priority': '优先级',
    'rules.mode': '模式',
    'rules.action': '执行动作',
    'rules.enabled': '已启用',
    'rules.disabled': '已禁用',
    'rules.testRule': '测试规则',
    'rules.testResults': '测试结果',
    'rules.deleteRule': '删除规则',
    'audit.title': '审计日志与历史',
    'audit.subtitle': '跟踪所有文件操作历史并执行撤销 (Undo)。',
    'audit.noLogs': '未记录任何审计日志。',
    'audit.action': '操作类别',
    'audit.file': '文件',
    'audit.dest': '目标路径',
    'audit.undo': '撤销操作',
    'settings.title': '应用设置',
    'settings.subtitle': '配置文件夹监控目标以及应用核心行为。',
    'settings.watchTargets': '监控目标文件夹',
    'settings.addNewTarget': '添加监控目标',
    'settings.noTargets': '未配置任何监控目标。',
    'settings.path': '文件夹路径',
    'settings.recursive': '包含子文件夹',
    'settings.defaultTtl': '默认过期时间 (秒)',
    'settings.ignorePatterns': '排除规则 (英文逗号分隔)',
    'settings.save': '保存全局配置',
    'settings.saved': '配置已成功保存。',
    'settings.general': '通用设置',
    'settings.protectedPatterns': '受保护的文件模式',
    'settings.safeFolder': '安全文件夹路径',
    'settings.notifications': '启用桌面通知',
    'settings.startup': '开机自动启动',
    'dialog.confirmTitle': '确认操作',
    'dialog.confirmText': '您确定要执行该操作吗？',
    'dialog.yes': '确定',
    'dialog.no': '取消',
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
