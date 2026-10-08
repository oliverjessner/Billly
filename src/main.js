import { initDropdowns, initTabs } from '/public/vendor/oj-designsystem/index.js';

const tauri = window.__TAURI__;
const invoke =
    tauri?.core?.invoke ??
    (async () => {
        throw new Error('Tauri core.invoke is not available');
    });
const listen =
    tauri?.event?.listen ??
    (async () => {
        throw new Error('Tauri event.listen is not available');
    });
const Chart = globalThis.Chart;
const state = Object.seal({
    currentView: 'dashboard',
    currentCategory: 'revenue',
    invoiceYear: 'all',
    selectedInvoiceId: null,
    selectedCustomerId: null,
    selectedEmailTemplateId: null,
    selectedEmailRecipientId: null,
    emailUsingCustomDraft: false,
    emailSending: false,
    currentEmailSection: 'send',
    currentSettingsSection: 'auths',
    invoiceItems: [],
    customerItems: [],
    emailTemplates: [],
    emailRecipients: [],
    emailInvoices: [],
    emailSenderEmail: null,
    emailGmailConnected: false,
    sorting: {
        invoices: { key: 'invoice_date', direction: 'desc' },
        customers: { key: 'lifetime_value', direction: 'desc' },
    },
    charts: {
        monthly: null,
    },
});
const $ = selector => document.querySelector(selector);
const $$ = selector => Array.from(document.querySelectorAll(selector));

function setStatus(text, ok = true) {
    const indicator = $('#status-indicator');
    indicator.querySelector('.text').textContent = text;
    indicator.classList.toggle('oj-status-success', ok);
    indicator.classList.toggle('oj-status-danger', !ok);
}

function setPillState(element, text, tone = 'neutral') {
    if (!element) return;
    element.textContent = text;
    element.classList.remove('oj-badge-success', 'oj-badge-warning', 'oj-badge-danger', 'oj-badge-info');
    if (tone !== 'neutral') element.classList.add(`oj-badge-${tone}`);
}

function setActiveSettingsSection(section) {
    state.currentSettingsSection = section;
    $$('.settings-toggle-button').forEach(button => {
        if (button.dataset.settingsSection) {
            button.classList.toggle('active', button.dataset.settingsSection === section);
        }
    });
}

function setActiveEmailSection(section) {
    state.currentEmailSection = section;
    $$('[data-email-section]').forEach(button => {
        button.classList.toggle('active', button.dataset.emailSection === section);
    });
}

function formatCurrency(value) {
    const amount = typeof value === 'number' ? value : Number.parseFloat(value || '0');
    return new Intl.NumberFormat('en-US', {
        style: 'currency',
        currency: 'EUR',
        minimumFractionDigits: 2,
    }).format(amount);
}

function getErrorMessage(error) {
    if (typeof error === 'string') return error;
    if (error && typeof error.message === 'string') return error.message;
    return 'Unknown error';
}

function createTextNode(tagName, className, text) {
    const node = document.createElement(tagName);
    if (className) {
        node.className = className;
    }
    node.textContent = text;
    return node;
}

function createEmptyTableRow(colspan, text) {
    const row = document.createElement('tr');
    const cell = document.createElement('td');
    cell.colSpan = colspan;
    cell.className = 'oj-muted';
    cell.textContent = text;
    row.appendChild(cell);
    return row;
}

let statusToggleCount = 0;

function createStatusToggle(id, status) {
    const toggle = document.createElement('div');
    toggle.className = 'status-toggle oj-segmented';
    toggle.dataset.id = id;
    toggle.setAttribute('role', 'group');
    toggle.setAttribute('aria-label', 'Invoice status');
    toggle.addEventListener('click', event => event.stopPropagation());
    const groupName = `invoice-status-${++statusToggleCount}`;

    ['open', 'paid'].forEach(option => {
        const label = document.createElement('label');
        const input = document.createElement('input');
        input.type = 'radio';
        input.name = groupName;
        input.className = 'status-option';
        input.checked = status === option;
        input.value = option;
        input.dataset.status = option;
        label.append(input, createTextNode('span', '', option === 'open' ? 'Open' : 'Paid'));
        toggle.appendChild(label);
    });

    return toggle;
}

function getInvoiceCategoryLabel(category) {
    return category === 'payable' ? 'Payables' : 'Revenue';
}

function setActiveInvoiceCategory(category) {
    state.currentCategory = category;
    $$('#invoice-category .segment').forEach(segment => {
        segment.checked = segment.dataset.category === category;
    });
}

const MONTH_NAMES = [
    'January',
    'February',
    'March',
    'April',
    'May',
    'June',
    'July',
    'August',
    'September',
    'October',
    'November',
    'December',
];
const PAYABLE_TAG_OPTIONS = ['SaaS', 'Insurance', 'Membership', 'Marketing', 'Domain', 'Rent'];

function setFilterValue(trigger, value) {
    const menu = trigger.closest('[data-oj-dropdown]').querySelector('[data-oj-dropdown-menu]');
    const items = Array.from(menu.querySelectorAll('[role="menuitemradio"]'));
    const selected = items.find(item => item.dataset.ojValue === String(value));
    if (!selected) return;
    trigger.value = selected.dataset.ojValue;
    trigger.querySelector('[data-oj-filter-value]').textContent = selected.querySelector('span').textContent;
    items.forEach(item => {
        const checked = item === selected;
        item.setAttribute('aria-checked', String(checked));
        item.querySelector('.filter-item-check').hidden = !checked;
    });
}

function renderFilterOptions(trigger, options, value) {
    const menu = trigger.closest('[data-oj-dropdown]').querySelector('[data-oj-dropdown-menu]');
    const items = options.map(option => {
        const item = document.createElement('button');
        item.type = 'button';
        item.className = 'oj-menu-item';
        item.setAttribute('role', 'menuitemradio');
        item.setAttribute('aria-checked', 'false');
        item.tabIndex = -1;
        item.dataset.ojValue = String(option.value);
        const check = document.createElement('i');
        check.className = 'filter-item-check fa-solid fa-check';
        check.setAttribute('aria-hidden', 'true');
        check.hidden = true;
        item.append(createTextNode('span', '', option.label), check);
        return item;
    });
    menu.replaceChildren(...items);
    trigger.disabled = !items.length;
    const selected = options.find(option => String(option.value) === String(value)) || options[0];
    if (selected) setFilterValue(trigger, selected.value);
}

function bindFilter(trigger, onChange) {
    const dropdown = trigger.closest('[data-oj-dropdown]');
    dropdown.addEventListener('oj:select', async event => {
        if (event.target !== dropdown || trigger.value === event.detail.value) return;
        setFilterValue(trigger, event.detail.value);
        try {
            await onChange();
        } catch (error) {
            console.error(error);
            setStatus(getErrorMessage(error), false);
        }
    });
}

async function loadAppVersion() {
    const versionText = $('#app-version-text');
    if (!versionText) return;
    try {
        const response = await fetch('/package.json');
        if (!response.ok) {
            throw new Error(`HTTP ${response.status}`);
        }
        const pkg = await response.json();
        versionText.textContent = `Version ${pkg.version || 'unknown'}`;
    } catch (error) {
        console.error(error);
        versionText.textContent = 'Version unavailable';
    }
}

function getSelectedEmailTemplate() {
    return state.emailTemplates.find(template => template.id === state.selectedEmailTemplateId) || null;
}

function getSelectedEmailRecipient() {
    return state.emailRecipients.find(recipient => recipient.id === state.selectedEmailRecipientId) || null;
}

function getSelectedEmailInvoice() {
    return state.emailInvoices.find(item => item.id === $('#email-invoice-select')?.value) || null;
}

function getEmailTemplateDateParts(invoice) {
    const raw = invoice?.invoice_date || '';
    const match = /^(\d{4})-(\d{2})-\d{2}$/.exec(raw);
    if (!match) {
        return { month: '', year: '' };
    }

    const year = match[1];
    const monthIndex = Number.parseInt(match[2], 10) - 1;
    return {
        month: MONTH_NAMES[monthIndex] || '',
        year,
    };
}

function renderEmailTemplatePlaceholders(value, recipient, invoice = getSelectedEmailInvoice()) {
    const { month, year } = getEmailTemplateDateParts(invoice);
    return String(value || '')
        .replaceAll('<name>', recipient?.display_name || '')
        .replaceAll('<email>', recipient?.email || '')
        .replaceAll('<month>', month)
        .replaceAll('<year>', year);
}

function getFilteredEmailInvoices(recipient = getSelectedEmailRecipient()) {
    if (!recipient) {
        return [...state.emailInvoices];
    }
    const filtered = state.emailInvoices.filter(invoice => invoice.customer_id === recipient.customer_id);
    return filtered.length ? filtered : [...state.emailInvoices];
}

function getEmailInvoiceLabel(invoice) {
    const parts = [];
    if (invoice.invoice_number) parts.push(invoice.invoice_number);
    if (invoice.invoice_date) parts.push(invoice.invoice_date);
    if (invoice.customer_name) parts.push(invoice.customer_name);
    parts.push(formatCurrency(invoice.total_amount));
    return parts.join(' • ');
}

function getEmailRecipientDisplayLabel(recipient) {
    if (!recipient) return '';
    return `${recipient.display_name} • ${recipient.email}`;
}

function monthNameFromValue(value) {
    if (!value) return 'Month';
    const index = Number.parseInt(value, 10) - 1;
    return MONTH_NAMES[index] || 'Month';
}

function formatCustomerAge(months) {
    const value = Number.parseInt(months || 0, 10);
    if (!Number.isFinite(value) || value <= 1) {
        return '1 month';
    }
    return `${value} months`;
}

function formatDaysSinceLastInvoice(value) {
    if (!value) return '-';
    const diffDays = getDaysSinceDate(value);
    if (diffDays == null) {
        return '-';
    }
    if (diffDays === 0) return 'Today';
    if (diffDays === 1) return '1 day';
    return `${diffDays} days`;
}

function getDaysSinceDate(value) {
    if (!value) return null;
    const parsed = new Date(`${value}T00:00:00`);
    if (Number.isNaN(parsed.getTime())) {
        return null;
    }
    const today = new Date();
    today.setHours(0, 0, 0, 0);
    const diffMs = today.getTime() - parsed.getTime();
    return Math.max(0, Math.floor(diffMs / 86400000));
}

function parseSortableDate(value) {
    if (!value) return null;
    const parsed = new Date(`${value}T00:00:00`);
    return Number.isNaN(parsed.getTime()) ? null : parsed.getTime();
}

function compareNullableValues(left, right, direction) {
    const leftMissing = left == null || left === '';
    const rightMissing = right == null || right === '';
    if (leftMissing && rightMissing) return 0;
    if (leftMissing) return 1;
    if (rightMissing) return -1;
    if (left < right) return direction === 'asc' ? -1 : 1;
    if (left > right) return direction === 'asc' ? 1 : -1;
    return 0;
}

function compareText(left, right, direction) {
    return compareNullableValues(
        String(left || '').trim().toLowerCase(),
        String(right || '').trim().toLowerCase(),
        direction
    );
}

function compareNumber(left, right, direction) {
    const normalizedLeft = Number.isFinite(left) ? left : null;
    const normalizedRight = Number.isFinite(right) ? right : null;
    return compareNullableValues(normalizedLeft, normalizedRight, direction);
}

function compareDate(left, right, direction) {
    return compareNullableValues(parseSortableDate(left), parseSortableDate(right), direction);
}

function getDefaultSortDirection(table, key) {
    if (table === 'invoices') {
        return ['invoice_date', 'total_amount'].includes(key) ? 'desc' : 'asc';
    }
    if (table === 'customers') {
        return ['first_invoice_date', 'last_invoice_date', 'lifetime_value', 'days_since_last_invoice'].includes(key)
            ? 'desc'
            : 'asc';
    }
    return 'asc';
}

function sortItems(items, table) {
    const { key, direction } = state.sorting[table];
    return items
        .map((item, index) => ({ item, index }))
        .sort((left, right) => {
            const a = left.item;
            const b = right.item;
            let result = 0;

            if (table === 'invoices') {
                switch (key) {
                    case 'invoice_date':
                        result = compareDate(a.invoice_date, b.invoice_date, direction);
                        break;
                    case 'counterparty_name':
                        result = compareText(a.counterparty_name, b.counterparty_name, direction);
                        break;
                    case 'total_amount':
                        result = compareNumber(Number.parseFloat(a.total_amount), Number.parseFloat(b.total_amount), direction);
                        break;
                    case 'status':
                        result = compareText(a.status, b.status, direction);
                        break;
                    case 'file_path':
                        result = compareText(
                            a.file_path ? a.file_path.split('/').pop() : '',
                            b.file_path ? b.file_path.split('/').pop() : '',
                            direction
                        );
                        break;
                    default:
                        result = 0;
                }
            } else if (table === 'customers') {
                switch (key) {
                    case 'name':
                        result = compareText(a.name, b.name, direction);
                        break;
                    case 'first_invoice_date':
                        result = compareDate(a.first_invoice_date, b.first_invoice_date, direction);
                        break;
                    case 'last_invoice_date':
                        result = compareDate(a.last_invoice_date, b.last_invoice_date, direction);
                        break;
                    case 'lifetime_value':
                        result = compareNumber(a.lifetime_value, b.lifetime_value, direction);
                        break;
                    case 'days_since_last_invoice':
                        result = compareNumber(
                            getDaysSinceDate(a.last_invoice_date),
                            getDaysSinceDate(b.last_invoice_date),
                            direction
                        );
                        break;
                    default:
                        result = 0;
                }
            }

            return result || left.index - right.index;
        })
        .map(entry => entry.item);
}

function updateSortHeaders() {
    $$('.table-sort').forEach(button => {
        const table = button.dataset.table;
        const key = button.dataset.sortKey;
        const sort = state.sorting[table];
        const indicator = button.querySelector('.table-sort-indicator');
        const active = sort?.key === key;
        const header = button.closest('th');
        if (header) {
            header.setAttribute('aria-sort', active ? (sort.direction === 'asc' ? 'ascending' : 'descending') : 'none');
        }
        button.classList.toggle('is-active', active);
        button.dataset.direction = active ? sort.direction : '';
        if (indicator) {
            indicator.textContent = active ? (sort.direction === 'asc' ? '↑' : '↓') : '';
        }
    });
}

function setActiveView(view) {
    state.currentView = view;
    $$('.tab').forEach(tab => tab.classList.toggle('active', tab.dataset.view === view));
    $$('.view').forEach(section => section.classList.toggle('active', section.id === `view-${view}`));
}

async function loadDashboard() {
    const year = $('#year-select').value;
    const month = $('#month-select').value;
    const yearMonth = year && month ? `${year}-${month}` : null;
    const stats = await invoke('get_dashboard_stats', { yearMonth });
    const yearLabel = year || String(new Date().getFullYear());
    renderKpis(stats, yearLabel, month);
    renderMetricValue('#overdue-revenue', stats.overdue_revenue, { absolute: false, negativeWhenPositive: true });
    renderMetricValue('#overdue-payables', stats.overdue_payables, { absolute: true, negativeWhenPositive: true });
    renderDashboardInvoiceList('#open-revenue-list', stats.open_revenue_items, 'No open revenue');
    renderDashboardInvoiceList('#open-payables-list', stats.open_payables_items, 'No open payables');
    renderAggregateList('#best-customer-list', stats.best_customers, 'No revenue data');
    renderAggregateList('#highest-cost-list', stats.highest_costs, 'No payables data', { absolute: true });
    const bestCustomerTitle = $('#best-customer-title');
    if (bestCustomerTitle) {
        bestCustomerTitle.textContent = `Best Customer (${yearLabel})`;
    }
    const highestCostTitle = $('#highest-cost-title');
    if (highestCostTitle) {
        highestCostTitle.textContent = `Highest Cost (${yearLabel})`;
    }
    renderCharts(stats);
}

function renderKpis(stats, yearLabel, monthLabel) {
    const monthText = monthNameFromValue(monthLabel);
    const kpis = [
        {
            label: `Revenue (${monthText})`,
            value: formatCurrency(stats.revenue_month),
            amount: stats.revenue_month,
            kind: 'revenue',
        },
        {
            label: `Payables (${monthText})`,
            value: formatCurrency(Math.abs(stats.payable_month)),
            amount: stats.payable_month,
            kind: 'payables',
        },
        {
            label: `Profit (${monthText})`,
            value: formatCurrency(stats.profit_month),
            amount: stats.profit_month,
            kind: 'profit',
        },
        {
            label: `Revenue (${yearLabel})`,
            value: formatCurrency(stats.revenue_year),
            amount: stats.revenue_year,
            kind: 'revenue',
        },
        {
            label: `Payables (${yearLabel})`,
            value: formatCurrency(Math.abs(stats.payable_year)),
            amount: stats.payable_year,
            kind: 'payables',
        },
        {
            label: `Profit (${yearLabel})`,
            value: formatCurrency(stats.profit_year),
            amount: stats.profit_year,
            kind: 'profit',
        },
    ];

    const container = $('#kpi-grid');
    container.replaceChildren();
    kpis.forEach(kpi => {
        const card = document.createElement('div');
        card.className = 'kpi oj-panel oj-metric oj-metric-card';
        const numeric = kpi.amount;
        const valueClass =
            kpi.kind && typeof numeric === 'number'
                ? kpi.kind === 'payables'
                    ? numeric === 0
                        ? 'value'
                        : 'value negative'
                    : numeric >= 0
                      ? 'value positive'
                      : 'value negative'
                : 'value';
        card.append(
            createTextNode('div', 'label oj-metric-label', kpi.label),
            createTextNode('div', `${valueClass} oj-metric-value`, kpi.value)
        );
        container.appendChild(card);
    });
}

function renderDashboardInvoiceList(selector, items, emptyText = 'No data') {
    const list = $(selector);
    list.replaceChildren();
    if (!items.length) {
        list.appendChild(createTextNode('li', 'oj-list-item oj-muted', emptyText));
        return;
    }
    items.forEach(item => {
        const li = document.createElement('li');
        li.className = 'oj-list-item';
        const name = document.createElement('span');
        name.textContent = item.counterparty_name || 'Unknown';
        const amount = document.createElement('span');
        amount.className = 'oj-mono';
        amount.textContent = formatCurrency(item.total_amount);
        li.append(name, amount);
        list.appendChild(li);
    });
}

function renderAggregateList(selector, items, emptyText = 'No data', { absolute = false } = {}) {
    const list = $(selector);
    list.replaceChildren();
    if (!items.length) {
        list.appendChild(createTextNode('li', 'oj-list-item oj-muted', emptyText));
        return;
    }
    items.forEach(item => {
        const li = document.createElement('li');
        li.className = 'oj-list-item';
        const label = document.createElement('span');
        label.textContent = item.label || 'Unknown';
        const amount = document.createElement('span');
        amount.className = 'oj-mono';
        amount.textContent = formatCurrency(absolute ? Math.abs(item.total_amount) : item.total_amount);
        li.append(label, amount);
        list.appendChild(li);
    });
}

function renderMetricValue(selector, value, { absolute = false, negativeWhenPositive = false } = {}) {
    const element = $(selector);
    if (!element) return;
    const amount = absolute ? Math.abs(value) : value;
    element.textContent = formatCurrency(amount);
    element.classList.remove('positive', 'negative');
    if (negativeWhenPositive && amount > 0) {
        element.classList.add('negative');
    } else if (amount > 0) {
        element.classList.add('positive');
    }
}

function getChartColors() {
    const scope = $('.app-shell');
    const styles = getComputedStyle(scope);
    const probe = document.createElement('span');
    probe.hidden = true;
    scope.appendChild(probe);
    const resolve = token => {
        probe.style.color = `var(${token})`;
        return getComputedStyle(probe).color;
    };
    const colors = {
        revenue: resolve('--oj-success'),
        revenueFill: resolve('--oj-success-soft'),
        payables: resolve('--oj-danger'),
        payablesFill: resolve('--oj-danger-soft'),
        text: resolve('--oj-text'),
        muted: resolve('--oj-text-muted'),
        grid: resolve('--oj-border-subtle'),
        sansFont: styles.getPropertyValue('--oj-font-sans').trim(),
        monoFont: styles.getPropertyValue('--oj-font-mono').trim(),
    };
    probe.remove();
    return colors;
}

function renderCharts(stats) {
    if (!Chart) {
        console.error('Chart.js not loaded');
        return;
    }
    const colors = getChartColors();
    if (!state.charts.monthly) {
        state.charts.monthly = new Chart($('#chart-monthly'), {
            type: 'line',
            data: {
                labels: stats.chart_months,
                datasets: [
                    {
                        label: 'Revenue',
                        data: stats.chart_revenue,
                        borderColor: colors.revenue,
                        backgroundColor: colors.revenueFill,
                        tension: 0.3,
                    },
                    {
                        label: 'Payables',
                        data: stats.chart_payables,
                        borderColor: colors.payables,
                        backgroundColor: colors.payablesFill,
                        tension: 0.3,
                    },
                ],
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                plugins: {
                    legend: { labels: { color: colors.text, font: { family: colors.sansFont } }, onClick: () => {} },
                    tooltip: {
                        titleFont: { family: colors.sansFont },
                        bodyFont: { family: colors.monoFont },
                    },
                },
                scales: {
                    x: { ticks: { color: colors.muted, font: { family: colors.sansFont } }, grid: { color: colors.grid } },
                    y: { ticks: { color: colors.muted, font: { family: colors.monoFont } }, grid: { color: colors.grid } },
                },
            },
        });
    } else {
        state.charts.monthly.data.labels = stats.chart_months;
        state.charts.monthly.data.datasets[0].data = stats.chart_revenue;
        state.charts.monthly.data.datasets[1].data = stats.chart_payables;
        state.charts.monthly.data.datasets[0].borderColor = colors.revenue;
        state.charts.monthly.data.datasets[0].backgroundColor = colors.revenueFill;
        state.charts.monthly.data.datasets[1].borderColor = colors.payables;
        state.charts.monthly.data.datasets[1].backgroundColor = colors.payablesFill;
        state.charts.monthly.options.plugins.legend.labels.color = colors.text;
        state.charts.monthly.options.plugins.legend.labels.font.family = colors.sansFont;
        state.charts.monthly.options.plugins.tooltip.titleFont.family = colors.sansFont;
        state.charts.monthly.options.plugins.tooltip.bodyFont.family = colors.monoFont;
        Object.entries(state.charts.monthly.options.scales).forEach(([axis, scale]) => {
            scale.ticks.color = colors.muted;
            scale.ticks.font.family = axis === 'y' ? colors.monoFont : colors.sansFont;
            scale.grid.color = colors.grid;
        });
        state.charts.monthly.update();
    }
}

async function loadInvoices() {
    const yearFilter = $('#invoice-year');
    const yearValue = yearFilter ? yearFilter.value : state.invoiceYear;
    const year = yearValue && yearValue !== 'all' ? yearValue : null;
    const list = await invoke('get_invoices', { category: state.currentCategory, year });
    state.invoiceItems = list;
    renderInvoiceList();
}

async function loadInvoiceYears() {
    const filter = $('#invoice-year');
    if (!filter) return;
    const years = await invoke('get_invoice_years', { category: state.currentCategory });
    const currentYear = String(new Date().getFullYear());
    const yearOptions = years.includes(currentYear) ? [...years] : [currentYear, ...years];
    const current = state.invoiceYear || filter.value || currentYear;
    const selected = current === 'all' || yearOptions.includes(current)
        ? current
        : yearOptions.includes(currentYear) ? currentYear : 'all';
    renderFilterOptions(filter, [
        { value: 'all', label: 'All' },
        ...yearOptions.map(year => ({ value: year, label: year })),
    ], selected);
    state.invoiceYear = filter.value;
}

async function loadDashboardYears() {
    const filter = $('#year-select');
    if (!filter) return;

    const [revenueYears, payableYears] = await Promise.all([
        invoke('get_invoice_years', { category: 'revenue' }),
        invoke('get_invoice_years', { category: 'payable' }),
    ]);

    const years = Array.from(new Set([...revenueYears, ...payableYears]))
        .filter(Boolean)
        .sort((a, b) => Number.parseInt(b, 10) - Number.parseInt(a, 10));

    const currentYear = String(new Date().getFullYear());
    const previous = filter.value;
    const yearOptions = years.length ? years : [currentYear];

    const selected = yearOptions.includes(previous)
        ? previous
        : yearOptions.includes(currentYear) ? currentYear : yearOptions[0];
    renderFilterOptions(filter, yearOptions.map(year => ({ value: year, label: year })), selected);
}

function renderInvoiceList() {
    const tbody = $('#invoice-list');
    const items = sortItems(state.invoiceItems, 'invoices');
    const focusedStatus = captureStatusFocus(tbody);
    tbody.replaceChildren();
    updateSortHeaders();
    if (!items.length) {
        tbody.appendChild(createEmptyTableRow(5, 'No invoices'));
        return;
    }

    items.forEach(item => {
        const row = document.createElement('tr');
        row.dataset.id = item.id;
        setRowSelection(row, item.id === state.selectedInvoiceId);
        const dateCell = createTextNode('td', 'oj-mono', item.invoice_date || '-');
        const counterpartyCell = document.createElement('td');
        const selectButton = createTextNode('button', 'oj-button oj-button-ghost oj-button-compact row-select', item.counterparty_name || 'Unknown');
        selectButton.type = 'button';
        selectButton.setAttribute('aria-label', `View invoice ${item.invoice_number || item.counterparty_name || item.invoice_date || ''}`.trim());
        selectButton.addEventListener('click', event => {
            event.stopPropagation();
            selectInvoice(item.id);
        });
        counterpartyCell.appendChild(selectButton);
        const amountCell = createTextNode('td', 'oj-numeric oj-mono', formatCurrency(item.total_amount));
        const statusCell = document.createElement('td');
        statusCell.appendChild(createStatusToggle(item.id, item.status));
        const fileCell = createTextNode('td', 'oj-mono', item.file_path ? item.file_path.split('/').pop() : '-');
        row.append(dateCell, counterpartyCell, amountCell, statusCell, fileCell);
        row.addEventListener('click', () => selectInvoice(item.id));
        row.querySelectorAll('.status-option').forEach(button => {
            button.addEventListener('change', async event => {
                event.stopPropagation();
                const status = button.dataset.status;
                await invoke('update_invoice_field', {
                    payload: {
                        invoiceId: item.id,
                        fieldName: 'status',
                        value: status,
                    },
                });
                await loadInvoices();
                if (state.selectedInvoiceId === item.id) {
                    await selectInvoice(item.id);
                }
                await loadDashboard();
            });
        });
        tbody.appendChild(row);
    });
    restoreStatusFocus(tbody, focusedStatus);
}

function captureStatusFocus(container) {
    const active = document.activeElement;
    if (!container.contains(active) || !active.matches('.status-option')) return null;
    return { id: active.closest('.status-toggle').dataset.id, status: active.dataset.status };
}

function restoreStatusFocus(container, focus) {
    if (!focus) return;
    const input = Array.from(container.querySelectorAll('.status-option')).find(option =>
        option.closest('.status-toggle').dataset.id === focus.id && option.dataset.status === focus.status
    );
    input?.focus({ preventScroll: true });
}

function setRowSelection(row, selected) {
    row.classList.toggle('selected', selected);
    if (selected) row.dataset.ojState = 'selected';
    else delete row.dataset.ojState;
}

async function selectInvoice(id) {
    state.selectedInvoiceId = id;
    $$('#invoice-list tr').forEach(row => {
        setRowSelection(row, row.dataset.id === id);
    });
    const detail = await invoke('get_invoice_detail', { invoiceId: id });
    renderInvoiceDetail(detail);
}

function renderInvoiceDetail(detail) {
    const container = $('#detail-body');
    const focusedStatus = captureStatusFocus(container);
    const invoice = detail.invoice;
    const fields = [
        ['invoice_number', 'Invoice Number'],
        ['invoice_date', 'Invoice Date'],
        ['due_date', 'Due Date'],
        ['counterparty_name', 'Company'],
        ['total_amount', 'Total Amount'],
        ['currency', 'Currency'],
        ['tax_amount', 'Tax'],
        ['net_amount', 'Net Amount'],
        ['send_date', 'Send Date', true],
        ['paid_at', 'Paid At'],
        ['document_language', 'Language', true],
        ['confidence_score', 'Confidence Score', true],
    ];

    container.replaceChildren();
    container.appendChild(createInvoiceCategoryRow(invoice));
    fields.forEach(([key, label, readOnly]) => {
        const row = document.createElement('div');
        row.className = 'detail-row';
        const labelNode = createTextNode('label', 'oj-label', label);
        const valueNode = document.createElement('div');
        valueNode.className = 'detail-value';
        const input = document.createElement('input');
        input.className = 'oj-input';
        input.id = `invoice-${invoice.id}-${key}`;
        labelNode.htmlFor = input.id;
        input.dataset.field = key;
        input.value = invoice[key] ?? '';
        if (readOnly) {
            input.readOnly = true;
        }
        valueNode.appendChild(input);
        row.append(labelNode, valueNode);
        container.appendChild(row);
    });

    const statusRow = document.createElement('div');
    statusRow.className = 'detail-row';
    const statusLabel = createTextNode('span', 'oj-label', 'Status');
    const statusValue = document.createElement('div');
    statusValue.className = 'detail-value';
    statusValue.appendChild(createStatusToggle(invoice.id, invoice.status));
    statusRow.append(statusLabel, statusValue);
    container.appendChild(statusRow);

    statusRow.querySelectorAll('.status-option').forEach(button => {
        button.addEventListener('change', async () => {
            const status = button.dataset.status;
            await invoke('update_invoice_field', {
                payload: {
                    invoiceId: invoice.id,
                    fieldName: 'status',
                    value: status,
                },
            });
            await selectInvoice(invoice.id);
            await loadInvoices();
            await loadDashboard();
        });
    });

    if (invoice.category === 'payable') {
        container.appendChild(createPayableTagsRow(invoice));
    }

    const ocrBlock = document.createElement('div');
    ocrBlock.className = 'detail-block';
    ocrBlock.append(
        createTextNode('h4', 'oj-heading-4', 'OCR / Extracted Text'),
        createTextNode('pre', 'oj-mono', invoice.ocr_text || 'No text stored')
    );
    container.appendChild(ocrBlock);

    const jsonBlock = document.createElement('div');
    jsonBlock.className = 'detail-block';
    jsonBlock.append(createTextNode('h4', 'oj-heading-4', 'Extracted JSON'), createTextNode('pre', 'oj-mono', invoice.extracted_json || '{}'));
    container.appendChild(jsonBlock);

    container.querySelectorAll('input[data-field]').forEach(input => {
        input.addEventListener('change', async event => {
            if (event.target.hasAttribute('readonly')) {
                return;
            }
            const field = event.target.dataset.field;
            await invoke('update_invoice_field', {
                payload: {
                    invoiceId: invoice.id,
                    fieldName: field,
                    value: event.target.value,
                },
            });
            await loadInvoices();
            await loadDashboard();
        });
    });
    restoreStatusFocus(container, focusedStatus);
}

function createInvoiceCategoryRow(invoice) {
    const row = document.createElement('div');
    row.className = 'detail-row detail-row-category';

    const label = createTextNode('span', 'oj-label', 'Category');
    const value = document.createElement('div');
    value.className = 'detail-value category-switch-field';

    const revenueLabel = createTextNode('span', 'category-switch-label', 'Revenue');
    const payableLabel = createTextNode('span', 'category-switch-label', 'Payables');

    const switchLabel = document.createElement('label');
    switchLabel.className = 'category-switch oj-switch';

    const input = document.createElement('input');
    input.type = 'checkbox';
    input.checked = invoice.category === 'payable';
    input.setAttribute('role', 'switch');
    input.setAttribute('aria-label', 'Move invoice between Revenue and Payables');

    const track = document.createElement('span');
    track.className = 'category-switch-track oj-switch-track';
    track.setAttribute('aria-hidden', 'true');

    input.addEventListener('change', async () => {
        const category = input.checked ? 'payable' : 'revenue';
        if (category === invoice.category) {
            return;
        }

        input.disabled = true;
        try {
            await moveInvoiceToCategory(invoice.id, category);
        } catch (error) {
            console.error(error);
            input.checked = invoice.category === 'payable';
            setStatus(getErrorMessage(error), false);
        } finally {
            input.disabled = false;
        }
    });

    switchLabel.append(input, track);
    value.append(revenueLabel, switchLabel, payableLabel);
    row.append(label, value);
    return row;
}

async function moveInvoiceToCategory(invoiceId, category) {
    await invoke('update_invoice_category', {
        payload: {
            invoiceId,
            category,
        },
    });

    state.selectedInvoiceId = invoiceId;
    setActiveInvoiceCategory(category);
    await loadDashboardYears();
    await loadDashboard();
    await loadInvoiceYears();
    await loadInvoices();
    await selectInvoice(invoiceId);
    await loadCustomers();
    await loadEmails();
    setStatus(`Invoice moved to ${getInvoiceCategoryLabel(category)}`, true);
}

function createPayableTagsRow(invoice) {
    const row = document.createElement('div');
    row.className = 'detail-row detail-row-tags';

    const label = document.createElement('span');
    label.className = 'oj-label';
    label.textContent = 'Tags';

    const editor = document.createElement('div');
    editor.className = 'detail-value detail-tags-editor';

    const activeTags = Array.isArray(invoice.tags) ? invoice.tags : [];
    const activeList = document.createElement('div');
    activeList.className = 'tag-list';

    if (activeTags.length) {
        activeTags.forEach(tag => {
            const button = document.createElement('button');
            button.type = 'button';
            button.className = 'tag-chip oj-tag';
            button.setAttribute('aria-pressed', 'true');
            button.setAttribute('aria-label', `Remove tag ${tag}`);
            button.textContent = `${tag} ×`;
            button.addEventListener('click', async () => {
                await saveInvoiceTags(invoice.id, activeTags.filter(value => value !== tag));
            });
            activeList.appendChild(button);
        });
    } else {
        const empty = document.createElement('span');
        empty.className = 'oj-muted';
        empty.textContent = 'No tags';
        activeList.appendChild(empty);
    }

    const availableList = document.createElement('div');
    availableList.className = 'tag-list tag-list-available';
    PAYABLE_TAG_OPTIONS.filter(tag => !activeTags.includes(tag)).forEach(tag => {
        const button = document.createElement('button');
        button.type = 'button';
        button.className = 'tag-chip oj-tag';
        button.setAttribute('aria-pressed', 'false');
        button.setAttribute('aria-label', `Add tag ${tag}`);
        button.textContent = `+ ${tag}`;
        button.addEventListener('click', async () => {
            await saveInvoiceTags(invoice.id, [...activeTags, tag]);
        });
        availableList.appendChild(button);
    });

    editor.append(activeList, availableList);
    row.append(label, editor);
    return row;
}

async function saveInvoiceTags(invoiceId, tags) {
    await invoke('update_invoice_tags', {
        payload: {
            invoiceId,
            tags,
        },
    });
    await selectInvoice(invoiceId);
    await loadInvoices();
    await loadDashboard();
}

function renderInvoicePlaceholder() {
    const container = $('#detail-body');
    container.replaceChildren(createTextNode('p', 'muted', 'Select an invoice from the list.'));
}

async function loadCustomers(options = {}) {
    const showDetail = options.showDetail ?? state.currentView === 'customers';
    const customers = await invoke('get_customers');
    state.customerItems = customers;
    renderCustomerList();
    const sortedCustomers = sortItems(customers, 'customers');

    const addContactButton = $('#add-customer-contact');
    if (!sortedCustomers.length) {
        state.selectedCustomerId = null;
        addContactButton.disabled = true;
        if (showDetail) {
            renderCustomerPlaceholder();
        }
        return;
    }

    const selectedExists = sortedCustomers.some(item => item.id === state.selectedCustomerId);
    if (!selectedExists) {
        state.selectedCustomerId = sortedCustomers[0].id;
    }

    addContactButton.disabled = !state.selectedCustomerId || !showDetail;
    if (showDetail && state.selectedCustomerId) {
        await selectCustomer(state.selectedCustomerId);
    }
}

function renderCustomerList() {
    const tbody = $('#customer-list');
    const items = sortItems(state.customerItems, 'customers');
    tbody.replaceChildren();
    updateSortHeaders();
    if (!items.length) {
        tbody.appendChild(createEmptyTableRow(5, 'No customers yet'));
        return;
    }

    items.forEach(item => {
        const row = document.createElement('tr');
        row.dataset.id = item.id;
        setRowSelection(row, item.id === state.selectedCustomerId);
        const nameCell = document.createElement('td');
        const selectButton = createTextNode('button', 'oj-button oj-button-ghost oj-button-compact row-select', item.name || 'Unknown');
        selectButton.type = 'button';
        selectButton.setAttribute('aria-label', `View customer ${item.name || 'Unknown'}`);
        selectButton.addEventListener('click', event => {
            event.stopPropagation();
            selectCustomer(item.id);
        });
        nameCell.appendChild(selectButton);
        row.append(
            nameCell,
            createTextNode('td', 'oj-mono', item.first_invoice_date || '-'),
            createTextNode('td', 'oj-mono', item.last_invoice_date || '-'),
            createTextNode('td', 'oj-numeric oj-mono', formatCurrency(item.lifetime_value)),
            createTextNode('td', 'oj-numeric', formatDaysSinceLastInvoice(item.last_invoice_date))
        );
        row.addEventListener('click', () => selectCustomer(item.id));
        tbody.appendChild(row);
    });
}

async function selectCustomer(id) {
    state.selectedCustomerId = id;
    $$('#customer-list tr').forEach(row => {
        setRowSelection(row, row.dataset.id === id);
    });
    $('#add-customer-contact').disabled = false;
    const detail = await invoke('get_customer_detail', { customerId: id });
    renderCustomerDetail(detail);
}

function renderCustomerPlaceholder() {
    const container = $('#customer-detail-body');
    container.replaceChildren(createTextNode('p', 'muted', 'Select a customer from the list.'));
}

function renderCustomerDetail(detail) {
    const container = $('#customer-detail-body');
    const customer = detail.customer;
    const contacts = detail.contacts || [];

    const metrics = document.createElement('div');
    metrics.className = 'detail-metrics';
    [
        ['Lifetime Value', formatCurrency(customer.lifetime_value)],
        ['Days Since Last Invoice', formatDaysSinceLastInvoice(customer.last_invoice_date)],
        ['Relationship', formatCustomerAge(customer.active_months)],
        ['Invoices', customer.invoice_count || 0],
    ].forEach(([label, value]) => {
        const card = document.createElement('div');
        card.className = 'detail-metric-card oj-metric oj-metric-card';
        card.append(createTextNode('span', 'detail-metric-label oj-metric-label', label), createTextNode('strong', 'oj-metric-value', value));
        metrics.appendChild(card);
    });
    container.replaceChildren(metrics);

    container.appendChild(createCustomerFieldRow('Customer Name', 'name', customer.name || ''));
    container.appendChild(createCustomerFieldRow('Main Email', 'general_email', customer.general_email || ''));
    container.appendChild(createCustomerFieldRow('Billing Address', 'billing_address', customer.billing_address || '', true));
    container.appendChild(createCustomerReadonlyRow('Generated From', customer.source_name || '-'));
    container.appendChild(createCustomerReadonlyRow('Customer Since', customer.first_invoice_date || '-'));
    container.appendChild(createCustomerReadonlyRow('Last Invoice', customer.last_invoice_date || '-'));
    container.appendChild(createCustomerReadonlyRow('AI Enrichment', customer.enrichment_status || 'pending'));

    const contactsBlock = document.createElement('div');
    contactsBlock.className = 'detail-block detail-contacts-block';
    const contactsHeader = document.createElement('div');
    contactsHeader.className = 'detail-block-header';
    contactsHeader.append(createTextNode('h4', 'oj-heading-4', 'Contacts'), createTextNode('span', 'oj-muted', `${contacts.length} saved`));
    contactsBlock.appendChild(contactsHeader);

    const contactsList = document.createElement('div');
    contactsList.className = 'contact-list';

    if (!contacts.length) {
        const empty = document.createElement('p');
        empty.className = 'oj-muted';
        empty.textContent = 'No contacts yet.';
        contactsList.appendChild(empty);
    } else {
        contacts.forEach(contact => {
            contactsList.appendChild(createCustomerContactRow(customer.id, contact));
        });
    }

    contactsBlock.appendChild(contactsList);
    container.appendChild(contactsBlock);

    container.querySelectorAll('[data-customer-field]').forEach(input => {
        input.addEventListener('change', async event => {
            const field = event.target.dataset.customerField;
            const value = event.target.value;
            await invoke('update_customer_field', {
                payload: {
                    customerId: customer.id,
                    fieldName: field,
                    value,
                },
            });
            await loadCustomers();
        });
    });
}

function createCustomerFieldRow(label, field, value, multiline = false) {
    const row = document.createElement('div');
    row.className = `detail-row ${multiline ? 'detail-row-multiline' : ''}`;

    const labelNode = document.createElement('label');
    labelNode.className = 'oj-label';
    labelNode.textContent = label;

    const valueNode = document.createElement('div');
    valueNode.className = 'detail-value';

    const input = multiline ? document.createElement('textarea') : document.createElement('input');
    input.className = multiline ? 'detail-textarea oj-textarea' : 'oj-input';
    input.id = `customer-${state.selectedCustomerId}-${field}`;
    if (field === 'general_email') input.type = 'email';
    labelNode.htmlFor = input.id;
    input.dataset.customerField = field;
    input.value = value;
    if (multiline) {
        input.rows = 3;
    }

    valueNode.appendChild(input);
    row.append(labelNode, valueNode);
    return row;
}

function createCustomerReadonlyRow(label, value) {
    const row = document.createElement('div');
    row.className = 'detail-row';
    row.append(createTextNode('span', 'oj-label', label), createTextNode('div', 'detail-value detail-value-static', value));
    return row;
}

function createCustomerContactRow(customerId, contact) {
    const row = document.createElement('div');
    row.className = 'contact-row';

    const nameInput = document.createElement('input');
    nameInput.className = 'oj-input';
    nameInput.id = `contact-${contact.id}-name`;
    nameInput.placeholder = 'Name';
    nameInput.value = contact.name || '';

    const emailInput = document.createElement('input');
    emailInput.className = 'oj-input';
    emailInput.id = `contact-${contact.id}-email`;
    emailInput.placeholder = 'email@company.com';
    emailInput.type = 'email';
    emailInput.value = contact.email || '';

    const removeButton = document.createElement('button');
    removeButton.type = 'button';
    removeButton.className = 'oj-button oj-button-ghost';
    removeButton.textContent = 'Remove';

    nameInput.addEventListener('change', async () => {
        await invoke('update_customer_contact_field', {
            payload: {
                contactId: contact.id,
                fieldName: 'name',
                value: nameInput.value,
            },
        });
        await loadCustomers();
    });

    emailInput.addEventListener('change', async () => {
        await invoke('update_customer_contact_field', {
            payload: {
                contactId: contact.id,
                fieldName: 'email',
                value: emailInput.value,
            },
        });
        await loadCustomers();
    });

    removeButton.addEventListener('click', async () => {
        await invoke('delete_customer_contact', { contactId: contact.id });
        await loadCustomers();
    });

    const nameField = document.createElement('div');
    nameField.className = 'oj-field';
    const nameLabel = createTextNode('label', 'oj-label', 'Name');
    nameLabel.htmlFor = nameInput.id;
    nameField.append(nameLabel, nameInput);
    const emailField = document.createElement('div');
    emailField.className = 'oj-field';
    const emailLabel = createTextNode('label', 'oj-label', 'Email');
    emailLabel.htmlFor = emailInput.id;
    emailField.append(emailLabel, emailInput);
    row.append(nameField, emailField, removeButton);
    return row;
}

async function loadEmails(options = {}) {
    const preserveCompose = options.preserveCompose ?? true;
    const workspace = await invoke('get_email_workspace');

    state.emailTemplates = workspace.templates || [];
    state.emailRecipients = workspace.recipients || [];
    state.emailInvoices = workspace.invoices || [];
    state.emailSenderEmail = workspace.sender_email || null;
    state.emailGmailConnected = Boolean(workspace.gmail_connected);

    const signatureInput = $('#email-signature');
    const previousSignature = signatureInput.value;
    signatureInput.value = workspace.signature || '';
    if (preserveCompose && previousSignature && !workspace.signature) {
        signatureInput.value = previousSignature;
    }

    $('#email-from').value = workspace.sender_email || '';

    if (
        state.selectedEmailTemplateId &&
        !state.emailTemplates.some(template => template.id === state.selectedEmailTemplateId)
    ) {
        state.selectedEmailTemplateId = state.emailTemplates[0]?.id || null;
        state.emailUsingCustomDraft = false;
    } else if (!state.selectedEmailTemplateId && !state.emailUsingCustomDraft) {
        state.selectedEmailTemplateId = state.emailTemplates[0]?.id || null;
    }
    if (!state.emailRecipients.some(recipient => recipient.id === state.selectedEmailRecipientId)) {
        state.selectedEmailRecipientId = state.emailRecipients[0]?.id || null;
    }

    renderEmailTemplateList();
    renderEmailTemplateEditor();
    renderEmailTemplateSelect();
    renderEmailRecipientList();
    populateEmailInvoiceSelect();
    setActiveEmailSection(state.currentEmailSection);

    const composeSubject = $('#email-compose-subject');
    const composeBody = $('#email-compose-body');
    if (
        !preserveCompose ||
        ((!composeSubject.value || !composeSubject.value.trim()) && (!composeBody.value || !composeBody.value.trim()))
    ) {
        applySelectedTemplateToCompose();
    }

    updateEmailComposeMeta();
    updateEmailComposePreview();
    updateEmailSendButton();
}

function renderEmailRecipientList() {
    const container = $('#email-recipient-list');
    const query = ($('#email-recipient-search').value || '').trim().toLowerCase();
    const recipients = state.emailRecipients.filter(recipient => {
        if (!query) return true;
        return [recipient.customer_name, recipient.display_name, recipient.email, recipient.kind]
            .filter(Boolean)
            .some(value => String(value).toLowerCase().includes(query));
    });

    container.replaceChildren();
    if (!recipients.length) {
        container.appendChild(createTextNode('p', 'muted', 'No recipients found.'));
        return;
    }

    let currentGroup = null;
    let groupList = null;
    recipients.forEach(recipient => {
        if (currentGroup !== recipient.customer_name) {
            currentGroup = recipient.customer_name;
            const group = document.createElement('div');
            group.className = 'email-recipient-group';
            group.appendChild(createTextNode('div', 'email-recipient-group-title', recipient.customer_name));
            groupList = document.createElement('div');
            groupList.className = 'email-recipient-group-list';
            group.appendChild(groupList);
            container.appendChild(group);
        }

        const button = document.createElement('button');
        button.type = 'button';
        button.className = 'email-recipient-button oj-button oj-panel oj-panel-compact oj-panel-interactive';
        const selected = recipient.id === state.selectedEmailRecipientId;
        button.setAttribute('aria-pressed', String(selected));
        if (selected) button.dataset.ojState = 'selected';
        const copy = document.createElement('span');
        copy.className = 'email-recipient-copy';
        copy.append(createTextNode('strong', '', recipient.display_name), createTextNode('span', '', recipient.email));
        button.append(copy, createTextNode('span', 'email-recipient-kind oj-badge', recipient.kind));
        button.addEventListener('click', () => selectEmailRecipient(recipient.id));
        groupList.appendChild(button);
    });
}

function selectEmailRecipient(recipientId) {
    state.selectedEmailRecipientId = recipientId;
    renderEmailRecipientList();
    populateEmailInvoiceSelect();
    updateEmailComposeMeta();
    updateEmailComposePreview();
    updateEmailSendButton();
}

function renderEmailTemplateList() {
    const container = $('#email-template-list');
    container.replaceChildren();
    if (!state.emailTemplates.length) {
        container.appendChild(createTextNode('p', 'muted', 'No templates yet. Create one to reuse your invoice emails.'));
        return;
    }

    state.emailTemplates.forEach(template => {
        const button = document.createElement('button');
        button.type = 'button';
        button.className = 'email-template-button oj-button oj-panel oj-panel-compact oj-panel-interactive';
        const selected = template.id === state.selectedEmailTemplateId;
        button.setAttribute('aria-pressed', String(selected));
        if (selected) button.dataset.ojState = 'selected';
        button.append(createTextNode('strong', '', template.name), createTextNode('span', '', template.subject || 'No subject yet'));
        button.addEventListener('click', () => selectEmailTemplate(template.id));
        container.appendChild(button);
    });
}

function renderEmailTemplateEditor() {
    const template = getSelectedEmailTemplate();
    $('#email-template-name').value = template?.name || '';
    $('#email-template-subject').value = template?.subject || '';
    $('#email-template-body').value = template?.body || '';
    $('#delete-email-template').disabled = !template;
}

function selectEmailTemplate(templateId, options = {}) {
    state.selectedEmailTemplateId = templateId;
    state.emailUsingCustomDraft = false;
    renderEmailTemplateList();
    renderEmailTemplateEditor();
    renderEmailTemplateSelect();
    if (options.loadIntoCompose !== false) {
        applySelectedTemplateToCompose();
    } else {
        updateEmailComposeMeta();
        updateEmailComposePreview();
        updateEmailSendButton();
    }
}

function applySelectedTemplateToCompose() {
    const template = getSelectedEmailTemplate();
    $('#email-compose-subject').value = template?.subject || '';
    $('#email-compose-body').value = template?.body || '';
    renderEmailTemplateSelect();
    updateEmailComposeMeta();
    updateEmailComposePreview();
    updateEmailSendButton();
}

function resetEmailTemplateEditor() {
    state.selectedEmailTemplateId = null;
    renderEmailTemplateList();
    renderEmailTemplateEditor();
    renderEmailTemplateSelect();
}

function activateCustomEmailDraft() {
    state.selectedEmailTemplateId = null;
    state.emailUsingCustomDraft = true;
    $('#email-compose-subject').value = '';
    $('#email-compose-body').value = '';
    renderEmailTemplateList();
    renderEmailTemplateEditor();
    renderEmailTemplateSelect();
    updateEmailComposeMeta();
    updateEmailComposePreview();
    updateEmailSendButton();
}

function renderEmailTemplateSelect() {
    const select = $('#email-template-select');
    const current = state.selectedEmailTemplateId;
    select.replaceChildren();

    const customOption = document.createElement('option');
    customOption.value = '';
    customOption.textContent = 'Custom draft';
    select.appendChild(customOption);

    state.emailTemplates.forEach(template => {
        const option = document.createElement('option');
        option.value = template.id;
        option.textContent = template.name;
        select.appendChild(option);
    });

    if (state.emailUsingCustomDraft) {
        select.value = '';
    } else if (current && state.emailTemplates.some(template => template.id === current)) {
        select.value = current;
    } else {
        select.value = '';
    }
}

function setEmailSendingState(isSending) {
    state.emailSending = isSending;
    const button = $('#send-email');
    if (!button) return;
    button.classList.toggle('oj-button-loading', isSending);
    button.setAttribute('aria-busy', isSending ? 'true' : 'false');
    const label = button.querySelector('.button-label');
    if (label) {
        label.textContent = isSending ? 'Sending...' : 'Send via Gmail';
    }
}

function populateEmailInvoiceSelect() {
    const select = $('#email-invoice-select');
    const invoices = getFilteredEmailInvoices();
    const previous = select.value;

    select.replaceChildren();
    if (!invoices.length) {
        const option = document.createElement('option');
        option.value = '';
        option.textContent = 'No revenue invoices available';
        select.appendChild(option);
        select.disabled = true;
        updateEmailComposeMeta();
        updateEmailComposePreview();
        updateEmailSendButton();
        return;
    }

    invoices.forEach(invoice => {
        const option = document.createElement('option');
        option.value = invoice.id;
        option.textContent = getEmailInvoiceLabel(invoice);
        select.appendChild(option);
    });

    select.disabled = false;
    if (invoices.some(invoice => invoice.id === previous)) {
        select.value = previous;
    } else {
        select.value = invoices[0].id;
    }

    updateEmailComposeMeta();
    updateEmailComposePreview();
    updateEmailSendButton();
}

function updateEmailComposeMeta() {
    const recipient = getSelectedEmailRecipient();
    const recipientDisplay = $('#email-recipient-display');
    recipientDisplay.value = recipient ? getEmailRecipientDisplayLabel(recipient) : '';

    const invoice = state.emailInvoices.find(item => item.id === $('#email-invoice-select').value) || null;
    const meta = $('#email-preview-meta');
    if (!recipient && !invoice) {
        meta.textContent = 'Select a recipient and invoice to preview the final email';
        return;
    }

    const parts = [];
    if (recipient) parts.push(recipient.customer_name);
    if (invoice) parts.push(getEmailInvoiceLabel(invoice));
    meta.textContent = parts.join(' • ');
}

function updateEmailComposePreview() {
    const recipient = getSelectedEmailRecipient();
    const invoice = getSelectedEmailInvoice();
    const subjectRaw = $('#email-compose-subject').value || '';
    const bodyRaw = $('#email-compose-body').value || '';
    const signature = ($('#email-signature').value || '').trim();
    const subject = renderEmailTemplatePlaceholders(subjectRaw, recipient, invoice).trim();
    const body = renderEmailTemplatePlaceholders(bodyRaw, recipient, invoice).trim();
    const fullBody = body && signature ? `${body}\n\n${signature}` : body || signature;
    const preview = $('#email-preview');

    preview.textContent = [
        `From: ${$('#email-from').value || '-'}`,
        `To: ${recipient ? recipient.email : '-'}`,
        `Subject: ${subject || '-'}`,
        '',
        fullBody || 'No message yet.',
    ].join('\n');
}

function updateEmailSendButton() {
    const button = $('#send-email');
    const hasRecipient = Boolean(getSelectedEmailRecipient());
    const hasInvoice = Boolean($('#email-invoice-select').value);
    const hasSubject = Boolean(($('#email-compose-subject').value || '').trim());
    button.disabled =
        state.emailSending || !state.emailGmailConnected || !state.emailSenderEmail || !hasRecipient || !hasInvoice || !hasSubject;
}

async function loadSettings() {
    const settings = await invoke('get_settings');
    $('#revenue-folder').value = settings.revenue_folder || '';
    $('#payable-folder').value = settings.payable_folder || '';
    $('#company-name').value = settings.company_name || '';
    $('#company-email').value = settings.company_email || '';
    $('#company-address').value = settings.company_address || '';
    const openAiInput = $('#openai-key');
    openAiInput.value = '';
    const savedKeyInput = $('#openai-key-saved');
    const removeButton = $('#remove-openai-key');
    if (settings.openai_api_key_present) {
        const suffix = settings.openai_api_key_last4 ? `Saved •••• ${settings.openai_api_key_last4}` : 'Saved key';
        setPillState(savedKeyInput, suffix, 'success');
        savedKeyInput.dataset.present = 'true';
        openAiInput.disabled = true;
        openAiInput.classList.add('disabled');
        removeButton.classList.remove('hidden');
    } else {
        setPillState(savedKeyInput, 'Not configured', 'neutral');
        savedKeyInput.dataset.present = 'false';
        openAiInput.disabled = false;
        openAiInput.classList.remove('disabled');
        removeButton.classList.add('hidden');
    }

    const googleStatus = $('#google-account-status');
    const gmailScopeStatus = $('#gmail-scope-status');
    const connectGoogleButton = $('#connect-google');
    const removeGoogleButton = $('#remove-google-account');
    const testGmailButton = $('#test-gmail');
    if (settings.google_connected) {
        setPillState(googleStatus, settings.google_email || 'Connected', 'success');
        setPillState(gmailScopeStatus, 'Send access granted', 'success');
        connectGoogleButton.textContent = 'Reconnect Google';
        removeGoogleButton.classList.remove('hidden');
        testGmailButton.disabled = false;
    } else {
        setPillState(googleStatus, 'Not connected', 'neutral');
        setPillState(gmailScopeStatus, 'Not granted', 'warning');
        connectGoogleButton.textContent = 'Connect Google';
        removeGoogleButton.classList.add('hidden');
        testGmailButton.disabled = true;
    }
}

async function saveSettings() {
    const payload = {
        revenueFolder: $('#revenue-folder').value || null,
        payableFolder: $('#payable-folder').value || null,
        companyName: $('#company-name').value || null,
        companyEmail: $('#company-email').value || null,
        companyAddress: $('#company-address').value || null,
        openaiApiKey: $('#openai-key').value || null,
    };
    await invoke('save_settings', { payload });
    await loadSettings();
    await loadDashboard();
    await loadInvoices();
}

async function init() {
    const now = new Date();
    const yearFilter = $('#year-select');
    const monthFilter = $('#month-select');

    const months = ['01', '02', '03', '04', '05', '06', '07', '08', '09', '10', '11', '12'];
    renderFilterOptions(monthFilter, months.map((value, index) => ({ value, label: MONTH_NAMES[index] })),
        String(now.getMonth() + 1).padStart(2, '0'));

    bindFilter(yearFilter, loadDashboard);
    bindFilter(monthFilter, loadDashboard);
    const invoiceYearFilter = $('#invoice-year');
    if (invoiceYearFilter) {
        bindFilter(invoiceYearFilter, async () => {
            state.invoiceYear = invoiceYearFilter.value;
            state.selectedInvoiceId = null;
            renderInvoicePlaceholder();
            await loadInvoices();
        });
    }

    const appTabs = $('.app-shell');
    const refreshView = async view => {
        if (view === 'customers') {
            await loadCustomers({ showDetail: true });
        } else if (view === 'emails') {
            await loadEmails();
        }
    };
    appTabs.addEventListener('oj:change', async event => {
        if (event.target !== appTabs) return;
        const view = event.detail.tab.dataset.view;
        setActiveView(view);
        await refreshView(view);
    });
    $$('.tab').forEach(tab => {
        tab.addEventListener('click', async () => {
            if (tab.getAttribute('aria-selected') === 'true') {
                await refreshView(tab.dataset.view);
            }
        });
    });

    const settingsTabs = $('.settings-shell');
    settingsTabs.addEventListener('oj:change', event => {
        if (event.target === settingsTabs) setActiveSettingsSection(event.detail.tab.dataset.settingsSection);
    });
    const emailTabs = $('.email-tabs');
    emailTabs.addEventListener('oj:change', event => {
        if (event.target === emailTabs) setActiveEmailSection(event.detail.tab.dataset.emailSection);
    });
    const cleanupTabs = initTabs(appTabs);
    const cleanupDropdowns = initDropdowns(appTabs);
    window.addEventListener('beforeunload', () => {
        cleanupDropdowns();
        cleanupTabs();
    }, { once: true });

    $$('#invoice-category .segment').forEach(segment => {
        segment.addEventListener('change', async () => {
            setActiveInvoiceCategory(segment.dataset.category);
            state.selectedInvoiceId = null;
            renderInvoicePlaceholder();
            await loadInvoiceYears();
            await loadInvoices();
        });
    });

    $$('.table-sort').forEach(button => {
        button.addEventListener('click', () => {
            const table = button.dataset.table;
            const key = button.dataset.sortKey;
            const currentSort = state.sorting[table];
            if (currentSort.key === key) {
                currentSort.direction = currentSort.direction === 'asc' ? 'desc' : 'asc';
            } else {
                currentSort.key = key;
                currentSort.direction = getDefaultSortDirection(table, key);
            }

            if (table === 'invoices') {
                renderInvoiceList();
            } else if (table === 'customers') {
                renderCustomerList();
            }
        });
    });

    $('#pick-revenue-folder').addEventListener('click', async () => {
        const selected = await invoke('pick_folder');
        if (selected) {
            $('#revenue-folder').value = selected;
        }
    });

    $('#pick-payable-folder').addEventListener('click', async () => {
        const selected = await invoke('pick_folder');
        if (selected) {
            $('#payable-folder').value = selected;
        }
    });

    $('#save-settings').addEventListener('click', saveSettings);
    $('#test-openai').addEventListener('click', async () => {
        const key = $('#openai-key').value;
        const savedPresent = $('#openai-key-saved').dataset.present === 'true';
        if (key && key.trim()) {
            const ok = await invoke('test_openai_key', { apiKey: key });
            setStatus(ok ? 'OpenAI key valid' : 'OpenAI key invalid', ok);
            return;
        }
        if (savedPresent) {
            const ok = await invoke('test_openai_key_saved');
            setStatus(ok ? 'Saved OpenAI key valid' : 'Saved OpenAI key invalid', ok);
            return;
        }
        setStatus('No OpenAI key to test', false);
    });

    $('#remove-openai-key').addEventListener('click', async () => {
        await invoke('remove_openai_key');
        await loadSettings();
        setStatus('OpenAI key removed', true);
    });

    $('#connect-google').addEventListener('click', async () => {
        try {
            setStatus('Opening Google login...', true);
            const email = await invoke('connect_google_account');
            await loadSettings();
            await loadEmails();
            setStatus(`Google connected: ${email}`, true);
        } catch (error) {
            console.error(error);
            await loadSettings();
            await loadEmails();
            setStatus(getErrorMessage(error), false);
        }
    });

    $('#remove-google-account').addEventListener('click', async () => {
        try {
            await invoke('remove_google_account');
            await loadSettings();
            await loadEmails();
            setStatus('Google account disconnected', true);
        } catch (error) {
            console.error(error);
            await loadSettings();
            await loadEmails();
            setStatus(getErrorMessage(error), false);
        }
    });

    $('#test-gmail').addEventListener('click', async () => {
        try {
            const email = await invoke('test_gmail_connection');
            await loadSettings();
            await loadEmails();
            setStatus(`Gmail ready: ${email}`, true);
        } catch (error) {
            console.error(error);
            await loadSettings();
            await loadEmails();
            setStatus(getErrorMessage(error), false);
        }
    });

    $('#email-recipient-search').addEventListener('input', renderEmailRecipientList);

    $('#email-template-select').addEventListener('change', event => {
        const templateId = event.target.value;
        if (!templateId) {
            activateCustomEmailDraft();
            return;
        }
        selectEmailTemplate(templateId);
    });

    $('#new-email-template').addEventListener('click', () => {
        resetEmailTemplateEditor();
        setStatus('New template draft ready', true);
    });

    $('#save-email-template').addEventListener('click', async () => {
        try {
            const saved = await invoke('save_email_template', {
                payload: {
                    templateId: state.selectedEmailTemplateId,
                    name: $('#email-template-name').value,
                    subject: $('#email-template-subject').value,
                    body: $('#email-template-body').value,
                },
            });
            state.selectedEmailTemplateId = saved.id;
            await loadEmails({ preserveCompose: false });
            setStatus(`Template saved: ${saved.name}`, true);
        } catch (error) {
            console.error(error);
            setStatus(getErrorMessage(error), false);
        }
    });

    $('#delete-email-template').addEventListener('click', async () => {
        if (!state.selectedEmailTemplateId) {
            setStatus('Select a template first', false);
            return;
        }
        try {
            await invoke('delete_email_template', { templateId: state.selectedEmailTemplateId });
            state.selectedEmailTemplateId = null;
            await loadEmails();
            setStatus('Template deleted', true);
        } catch (error) {
            console.error(error);
            setStatus(getErrorMessage(error), false);
        }
    });

    $('#save-email-signature').addEventListener('click', async () => {
        try {
            await invoke('save_email_signature', {
                payload: {
                    signature: $('#email-signature').value,
                },
            });
            updateEmailComposePreview();
            setStatus('Signature saved', true);
        } catch (error) {
            console.error(error);
            setStatus(getErrorMessage(error), false);
        }
    });

    $('#email-invoice-select').addEventListener('change', () => {
        updateEmailComposeMeta();
        updateEmailComposePreview();
        updateEmailSendButton();
    });

    $('#email-compose-subject').addEventListener('input', () => {
        updateEmailComposePreview();
        updateEmailSendButton();
    });

    $('#email-compose-body').addEventListener('input', () => {
        updateEmailComposePreview();
        updateEmailSendButton();
    });

    $('#email-signature').addEventListener('input', updateEmailComposePreview);

    $('#send-email').addEventListener('click', async () => {
        const recipient = getSelectedEmailRecipient();
        const invoiceId = $('#email-invoice-select').value;
        if (!recipient) {
            setStatus('Select a recipient first', false);
            return;
        }
        if (!invoiceId) {
            setStatus('Select an invoice first', false);
            return;
        }
        try {
            setEmailSendingState(true);
            updateEmailSendButton();
            const sentTo = await invoke('send_email', {
                payload: {
                    recipientId: recipient.id,
                    invoiceId,
                    subject: $('#email-compose-subject').value,
                    body: $('#email-compose-body').value,
                },
            });
            await loadEmails();
            setStatus(`Invoice email sent to ${sentTo}`, true);
        } catch (error) {
            console.error(error);
            await loadEmails();
            setStatus(getErrorMessage(error), false);
        } finally {
            setEmailSendingState(false);
            updateEmailSendButton();
        }
    });

    $('#add-customer-contact').addEventListener('click', async () => {
        if (!state.selectedCustomerId) {
            setStatus('Select a customer first', false);
            return;
        }
        await invoke('add_customer_contact', { customerId: state.selectedCustomerId });
        await loadCustomers();
        setStatus('Contact added', true);
    });

    $('#reprocess-single').addEventListener('click', async () => {
        if (!state.selectedInvoiceId) return;
        await invoke('reprocess_invoice', { invoiceId: state.selectedInvoiceId });
        setStatus('Invoice reprocessed', true);
    });

    $('#open-pdf').addEventListener('click', async () => {
        if (!state.selectedInvoiceId) return;
        const detail = await invoke('get_invoice_detail', { invoiceId: state.selectedInvoiceId });
        if (detail.invoice.file_path) {
            await invoke('open_invoice_file', { path: detail.invoice.file_path });
        }
    });

    $('#delete-invoice').addEventListener('click', async () => {
        if (!state.selectedInvoiceId) {
            setStatus('Select an invoice first', false);
            return;
        }
        try {
            await invoke('delete_invoice', { invoiceId: state.selectedInvoiceId });
            state.selectedInvoiceId = null;
            renderInvoicePlaceholder();
            await loadDashboardYears();
            await loadDashboard();
            await loadInvoiceYears();
            await loadInvoices();
            await loadCustomers();
            await loadEmails();
            setStatus('Invoice deleted', true);
        } catch (err) {
            console.error(err);
            setStatus('Failed to delete invoice', false);
        }
    });

    await loadSettings();
    setActiveSettingsSection(state.currentSettingsSection);
    await loadAppVersion();
    await loadDashboardYears();
    await loadDashboard();
    await loadInvoiceYears();
    await loadInvoices();
    await loadCustomers();
    await loadEmails();

    await listen('invoice-updated', async event => {
        await loadDashboardYears();
        await loadDashboard();
        await loadInvoiceYears();
        await loadInvoices();
        await loadCustomers();
        await loadEmails();
        const category = event?.payload?.category;
        if (category === 'revenue') {
            setStatus('Found new invoice', true);
        } else if (category === 'payable') {
            setStatus('Found new payable', true);
        }
    });
    await listen('processing-error', event => {
        setStatus(`Error: ${event.payload}`, false);
    });
}

init().catch(err => {
    console.error(err);
    setStatus('Startup error', false);
});
