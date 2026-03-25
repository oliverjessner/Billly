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
    indicator.querySelector('.dot').style.background = ok ? 'var(--ok)' : 'var(--danger)';
}

function setPillState(element, text, tone = 'neutral') {
    if (!element) return;
    element.textContent = text;
    element.classList.remove('is-success', 'is-warning', 'is-neutral');
    element.classList.add(`is-${tone}`);
}

function setActiveSettingsSection(section) {
    state.currentSettingsSection = section;
    $$('.settings-toggle-button').forEach(button => {
        if (button.dataset.settingsSection) {
            button.classList.toggle('active', button.dataset.settingsSection === section);
        }
    });
    $$('[data-settings-panel]').forEach(panel => {
        panel.classList.toggle('hidden', panel.dataset.settingsPanel !== section);
    });
}

function setActiveEmailSection(section) {
    state.currentEmailSection = section;
    $$('[data-email-section]').forEach(button => {
        button.classList.toggle('active', button.dataset.emailSection === section);
    });
    $$('[data-email-panel]').forEach(panel => {
        panel.classList.toggle('hidden', panel.dataset.emailPanel !== section);
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

function escapeHtml(value) {
    return String(value ?? '')
        .replaceAll('&', '&amp;')
        .replaceAll('<', '&lt;')
        .replaceAll('>', '&gt;');
}

function getErrorMessage(error) {
    if (typeof error === 'string') return error;
    if (error && typeof error.message === 'string') return error.message;
    return 'Unknown error';
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
    container.innerHTML = '';
    kpis.forEach(kpi => {
        const card = document.createElement('div');
        card.className = 'kpi';
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
        card.innerHTML = `<div class="label">${kpi.label}</div><div class="${valueClass}">${kpi.value}</div>`;
        container.appendChild(card);
    });
}

function renderDashboardInvoiceList(selector, items, emptyText = 'No data') {
    const list = $(selector);
    list.innerHTML = '';
    if (!items.length) {
        list.innerHTML = `<li class="muted">${emptyText}</li>`;
        return;
    }
    items.forEach(item => {
        const li = document.createElement('li');
        const name = document.createElement('span');
        name.textContent = item.counterparty_name || 'Unknown';
        const amount = document.createElement('span');
        amount.textContent = formatCurrency(item.total_amount);
        li.append(name, amount);
        list.appendChild(li);
    });
}

function renderAggregateList(selector, items, emptyText = 'No data', { absolute = false } = {}) {
    const list = $(selector);
    list.innerHTML = '';
    if (!items.length) {
        list.innerHTML = `<li class="muted">${emptyText}</li>`;
        return;
    }
    items.forEach(item => {
        const li = document.createElement('li');
        const label = document.createElement('span');
        label.textContent = item.label || 'Unknown';
        const amount = document.createElement('span');
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

function renderCharts(stats) {
    if (!Chart) {
        console.error('Chart.js not loaded');
        return;
    }
    if (!state.charts.monthly) {
        state.charts.monthly = new Chart($('#chart-monthly'), {
            type: 'line',
            data: {
                labels: stats.chart_months,
                datasets: [
                    {
                        label: 'Revenue',
                        data: stats.chart_revenue,
                        borderColor: '#22c55e',
                        backgroundColor: 'rgba(34,197,94,0.2)',
                        tension: 0.3,
                    },
                    {
                        label: 'Payables',
                        data: stats.chart_payables,
                        borderColor: '#ef4444',
                        backgroundColor: 'rgba(239,68,68,0.2)',
                        tension: 0.3,
                    },
                ],
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                plugins: {
                    legend: { labels: { color: '#f3f4f6' }, onClick: () => {} },
                },
                scales: {
                    x: { ticks: { color: '#9aa3b2' }, grid: { color: 'rgba(255,255,255,0.05)' } },
                    y: { ticks: { color: '#9aa3b2' }, grid: { color: 'rgba(255,255,255,0.05)' } },
                },
            },
        });
    } else {
        state.charts.monthly.data.labels = stats.chart_months;
        state.charts.monthly.data.datasets[0].data = stats.chart_revenue;
        state.charts.monthly.data.datasets[1].data = stats.chart_payables;
        state.charts.monthly.update();
    }
}

async function loadInvoices() {
    const yearSelect = $('#invoice-year');
    const yearValue = yearSelect ? yearSelect.value : state.invoiceYear;
    const year = yearValue && yearValue !== 'all' ? yearValue : null;
    const list = await invoke('get_invoices', { category: state.currentCategory, year });
    state.invoiceItems = list;
    renderInvoiceList();
}

async function loadInvoiceYears() {
    const select = $('#invoice-year');
    if (!select) return;
    const years = await invoke('get_invoice_years', { category: state.currentCategory });
    const currentYear = String(new Date().getFullYear());
    const yearOptions = years.includes(currentYear) ? [...years] : [currentYear, ...years];
    const current = state.invoiceYear || select.value || currentYear;
    select.innerHTML = '';
    const allOption = document.createElement('option');
    allOption.value = 'all';
    allOption.textContent = 'All';
    select.appendChild(allOption);
    yearOptions.forEach(year => {
        const option = document.createElement('option');
        option.value = year;
        option.textContent = year;
        select.appendChild(option);
    });
    if (current === 'all') {
        select.value = 'all';
    } else if (yearOptions.includes(current)) {
        select.value = current;
    } else if (yearOptions.includes(currentYear)) {
        select.value = currentYear;
    } else {
        select.value = 'all';
    }
    state.invoiceYear = select.value;
}

async function loadDashboardYears() {
    const select = $('#year-select');
    if (!select) return;

    const [revenueYears, payableYears] = await Promise.all([
        invoke('get_invoice_years', { category: 'revenue' }),
        invoke('get_invoice_years', { category: 'payable' }),
    ]);

    const years = Array.from(new Set([...revenueYears, ...payableYears]))
        .filter(Boolean)
        .sort((a, b) => Number.parseInt(b, 10) - Number.parseInt(a, 10));

    const currentYear = String(new Date().getFullYear());
    const previous = select.value;
    const yearOptions = years.length ? years : [currentYear];

    select.innerHTML = '';
    yearOptions.forEach(year => {
        const option = document.createElement('option');
        option.value = year;
        option.textContent = year;
        select.appendChild(option);
    });

    if (yearOptions.includes(previous)) {
        select.value = previous;
    } else if (yearOptions.includes(currentYear)) {
        select.value = currentYear;
    } else {
        select.value = yearOptions[0];
    }
}

function renderInvoiceList() {
    const tbody = $('#invoice-list');
    const items = sortItems(state.invoiceItems, 'invoices');
    tbody.innerHTML = '';
    updateSortHeaders();
    if (!items.length) {
        tbody.innerHTML = `<tr><td colspan="5" class="muted">No invoices</td></tr>`;
        return;
    }

    items.forEach(item => {
        const row = document.createElement('tr');
        row.dataset.id = item.id;
        row.classList.toggle('selected', item.id === state.selectedInvoiceId);
        row.innerHTML = `
      <td>${item.invoice_date || '-'}</td>
      <td>${item.counterparty_name || 'Unknown'}</td>
      <td>${formatCurrency(item.total_amount)}</td>
      <td>
        <div class="status-toggle" data-id="${item.id}">
          <button class="status-option ${item.status === 'open' ? 'active' : ''}" data-status="open">Open</button>
          <button class="status-option ${item.status === 'paid' ? 'active' : ''}" data-status="paid">Paid</button>
        </div>
      </td>
      <td>${item.file_path ? item.file_path.split('/').pop() : '-'}</td>
    `;
        row.addEventListener('click', () => selectInvoice(item.id));
        row.querySelectorAll('.status-option').forEach(button => {
            button.addEventListener('click', async event => {
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
}

async function selectInvoice(id) {
    state.selectedInvoiceId = id;
    $$('#invoice-list tr').forEach(row => {
        row.classList.toggle('selected', row.dataset.id === id);
    });
    const detail = await invoke('get_invoice_detail', { invoiceId: id });
    renderInvoiceDetail(detail);
}

function renderInvoiceDetail(detail) {
    const container = $('#detail-body');
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

    container.innerHTML = '';
    fields.forEach(([key, label, readOnly]) => {
        const row = document.createElement('div');
        row.className = 'detail-row';
        row.innerHTML = `
      <span>${label}</span>
      <div class="detail-value">
        <input data-field="${key}" value="${invoice[key] ?? ''}" ${readOnly ? 'readonly' : ''} />
      </div>
    `;
        container.appendChild(row);
    });

    const statusRow = document.createElement('div');
    statusRow.className = 'detail-row';
    statusRow.innerHTML = `
    <span>Status</span>
    <div class="detail-value">
      <div class="status-toggle" data-id="${invoice.id}">
        <button class="status-option ${invoice.status === 'open' ? 'active' : ''}" data-status="open">Open</button>
        <button class="status-option ${invoice.status === 'paid' ? 'active' : ''}" data-status="paid">Paid</button>
      </div>
    </div>
  `;
    container.appendChild(statusRow);

    statusRow.querySelectorAll('.status-option').forEach(button => {
        button.addEventListener('click', async () => {
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
    ocrBlock.innerHTML = `
    <h4>OCR / Extracted Text</h4>
    <pre>${escapeHtml(invoice.ocr_text || 'No text stored')}<\/pre>
  `;
    container.appendChild(ocrBlock);

    const jsonBlock = document.createElement('div');
    jsonBlock.className = 'detail-block';
    jsonBlock.innerHTML = `
    <h4>Extracted JSON</h4>
    <pre>${escapeHtml(invoice.extracted_json || '{}')}<\/pre>
  `;
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
}

function createPayableTagsRow(invoice) {
    const row = document.createElement('div');
    row.className = 'detail-row detail-row-tags';

    const label = document.createElement('span');
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
            button.className = 'tag-chip is-active';
            button.textContent = `${tag} ×`;
            button.addEventListener('click', async () => {
                await saveInvoiceTags(invoice.id, activeTags.filter(value => value !== tag));
            });
            activeList.appendChild(button);
        });
    } else {
        const empty = document.createElement('span');
        empty.className = 'muted';
        empty.textContent = 'No tags';
        activeList.appendChild(empty);
    }

    const availableList = document.createElement('div');
    availableList.className = 'tag-list tag-list-available';
    PAYABLE_TAG_OPTIONS.filter(tag => !activeTags.includes(tag)).forEach(tag => {
        const button = document.createElement('button');
        button.type = 'button';
        button.className = 'tag-chip is-add';
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
    container.innerHTML = `<p class="muted">Select an invoice from the list.</p>`;
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
    tbody.innerHTML = '';
    updateSortHeaders();
    if (!items.length) {
        tbody.innerHTML = `<tr><td colspan="5" class="muted">No customers yet</td></tr>`;
        return;
    }

    items.forEach(item => {
        const row = document.createElement('tr');
        row.dataset.id = item.id;
        row.classList.toggle('selected', item.id === state.selectedCustomerId);
        row.innerHTML = `
      <td>${escapeHtml(item.name || 'Unknown')}</td>
      <td>${escapeHtml(item.first_invoice_date || '-')}</td>
      <td>${escapeHtml(item.last_invoice_date || '-')}</td>
      <td>${formatCurrency(item.lifetime_value)}</td>
      <td>${formatDaysSinceLastInvoice(item.last_invoice_date)}</td>
    `;
        row.addEventListener('click', () => selectCustomer(item.id));
        tbody.appendChild(row);
    });
}

async function selectCustomer(id) {
    state.selectedCustomerId = id;
    $$('#customer-list tr').forEach(row => {
        row.classList.toggle('selected', row.dataset.id === id);
    });
    $('#add-customer-contact').disabled = false;
    const detail = await invoke('get_customer_detail', { customerId: id });
    renderCustomerDetail(detail);
}

function renderCustomerPlaceholder() {
    const container = $('#customer-detail-body');
    container.innerHTML = `<p class="muted">Select a customer from the list.</p>`;
}

function renderCustomerDetail(detail) {
    const container = $('#customer-detail-body');
    const customer = detail.customer;
    const contacts = detail.contacts || [];

    container.innerHTML = `
      <div class="detail-metrics">
        <div class="detail-metric-card">
          <span class="detail-metric-label">Lifetime Value</span>
          <strong>${formatCurrency(customer.lifetime_value)}</strong>
        </div>
        <div class="detail-metric-card">
          <span class="detail-metric-label">Days Since Last Invoice</span>
          <strong>${formatDaysSinceLastInvoice(customer.last_invoice_date)}</strong>
        </div>
        <div class="detail-metric-card">
          <span class="detail-metric-label">Relationship</span>
          <strong>${formatCustomerAge(customer.active_months)}</strong>
        </div>
        <div class="detail-metric-card">
          <span class="detail-metric-label">Invoices</span>
          <strong>${customer.invoice_count || 0}</strong>
        </div>
      </div>
    `;

    container.appendChild(createCustomerFieldRow('Customer Name', 'name', customer.name || ''));
    container.appendChild(createCustomerFieldRow('Main Email', 'general_email', customer.general_email || ''));
    container.appendChild(createCustomerFieldRow('Billing Address', 'billing_address', customer.billing_address || '', true));
    container.appendChild(createCustomerReadonlyRow('Generated From', customer.source_name || '-'));
    container.appendChild(createCustomerReadonlyRow('Customer Since', customer.first_invoice_date || '-'));
    container.appendChild(createCustomerReadonlyRow('Last Invoice', customer.last_invoice_date || '-'));
    container.appendChild(createCustomerReadonlyRow('AI Enrichment', customer.enrichment_status || 'pending'));

    const contactsBlock = document.createElement('div');
    contactsBlock.className = 'detail-block detail-contacts-block';
    contactsBlock.innerHTML = `
      <div class="detail-block-header">
        <h4>Contacts</h4>
        <span class="muted">${contacts.length} saved</span>
      </div>
    `;

    const contactsList = document.createElement('div');
    contactsList.className = 'contact-list';

    if (!contacts.length) {
        const empty = document.createElement('p');
        empty.className = 'muted';
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

    const labelNode = document.createElement('span');
    labelNode.textContent = label;

    const valueNode = document.createElement('div');
    valueNode.className = 'detail-value';

    const input = multiline ? document.createElement('textarea') : document.createElement('input');
    input.dataset.customerField = field;
    input.value = value;
    if (multiline) {
        input.className = 'detail-textarea';
        input.rows = 3;
    }

    valueNode.appendChild(input);
    row.append(labelNode, valueNode);
    return row;
}

function createCustomerReadonlyRow(label, value) {
    const row = document.createElement('div');
    row.className = 'detail-row';
    row.innerHTML = `
      <span>${escapeHtml(label)}</span>
      <div class="detail-value detail-value-static">${escapeHtml(value)}</div>
    `;
    return row;
}

function createCustomerContactRow(customerId, contact) {
    const row = document.createElement('div');
    row.className = 'contact-row';

    const nameInput = document.createElement('input');
    nameInput.placeholder = 'Name';
    nameInput.value = contact.name || '';

    const emailInput = document.createElement('input');
    emailInput.placeholder = 'email@company.com';
    emailInput.type = 'email';
    emailInput.value = contact.email || '';

    const removeButton = document.createElement('button');
    removeButton.type = 'button';
    removeButton.className = 'ghost';
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

    row.append(nameInput, emailInput, removeButton);
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

    container.innerHTML = '';
    if (!recipients.length) {
        container.innerHTML = `<p class="muted">No recipients found.</p>`;
        return;
    }

    let currentGroup = null;
    let groupList = null;
    recipients.forEach(recipient => {
        if (currentGroup !== recipient.customer_name) {
            currentGroup = recipient.customer_name;
            const group = document.createElement('div');
            group.className = 'email-recipient-group';
            group.innerHTML = `<div class="email-recipient-group-title">${escapeHtml(recipient.customer_name)}</div>`;
            groupList = document.createElement('div');
            groupList.className = 'email-recipient-group-list';
            group.appendChild(groupList);
            container.appendChild(group);
        }

        const button = document.createElement('button');
        button.type = 'button';
        button.className = 'email-recipient-button';
        button.classList.toggle('active', recipient.id === state.selectedEmailRecipientId);
        button.innerHTML = `
            <span class="email-recipient-copy">
                <strong>${escapeHtml(recipient.display_name)}</strong>
                <span>${escapeHtml(recipient.email)}</span>
            </span>
            <span class="email-recipient-kind">${escapeHtml(recipient.kind)}</span>
        `;
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
    container.innerHTML = '';
    if (!state.emailTemplates.length) {
        container.innerHTML = `<p class="muted">No templates yet. Create one to reuse your invoice emails.</p>`;
        return;
    }

    state.emailTemplates.forEach(template => {
        const button = document.createElement('button');
        button.type = 'button';
        button.className = 'email-template-button';
        button.classList.toggle('active', template.id === state.selectedEmailTemplateId);
        button.innerHTML = `
            <strong>${escapeHtml(template.name)}</strong>
            <span>${escapeHtml(template.subject || 'No subject yet')}</span>
        `;
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
    select.innerHTML = '';

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
    button.classList.toggle('is-loading', isSending);
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

    select.innerHTML = '';
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
    const yearSelect = $('#year-select');
    const monthSelect = $('#month-select');

    const months = ['01', '02', '03', '04', '05', '06', '07', '08', '09', '10', '11', '12'];
    months.forEach((value, index) => {
        const option = document.createElement('option');
        option.value = value;
        option.textContent = MONTH_NAMES[index] || value;
        monthSelect.appendChild(option);
    });
    monthSelect.value = String(now.getMonth() + 1).padStart(2, '0');

    yearSelect.addEventListener('change', loadDashboard);
    monthSelect.addEventListener('change', loadDashboard);
    const invoiceYearSelect = $('#invoice-year');
    if (invoiceYearSelect) {
        invoiceYearSelect.addEventListener('change', async () => {
            state.invoiceYear = invoiceYearSelect.value;
            state.selectedInvoiceId = null;
            renderInvoicePlaceholder();
            await loadInvoices();
        });
    }

    $$('.tab').forEach(tab => {
        tab.addEventListener('click', async () => {
            const view = tab.dataset.view;
            setActiveView(view);
            if (view === 'customers') {
                await loadCustomers({ showDetail: true });
            } else if (view === 'emails') {
                await loadEmails();
            }
        });
    });

    $$('.segment').forEach(segment => {
        segment.addEventListener('click', async () => {
            $$('.segment').forEach(s => s.classList.remove('active'));
            segment.classList.add('active');
            state.currentCategory = segment.dataset.category;
            await loadInvoiceYears();
            await loadInvoices();
        });
    });

    $$('.settings-toggle-button').forEach(button => {
        if (button.dataset.settingsSection) {
            button.addEventListener('click', () => {
                setActiveSettingsSection(button.dataset.settingsSection);
            });
        }
        if (button.dataset.emailSection) {
            button.addEventListener('click', () => {
                setActiveEmailSection(button.dataset.emailSection);
            });
        }
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
