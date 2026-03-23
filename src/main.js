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
    const parsed = new Date(`${value}T00:00:00`);
    if (Number.isNaN(parsed.getTime())) {
        return '-';
    }
    const today = new Date();
    today.setHours(0, 0, 0, 0);
    const diffMs = today.getTime() - parsed.getTime();
    const diffDays = Math.max(0, Math.floor(diffMs / 86400000));
    if (diffDays === 0) return 'Today';
    if (diffDays === 1) return '1 day';
    return `${diffDays} days`;
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
    renderInvoiceList(list);
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

function renderInvoiceList(items) {
    const tbody = $('#invoice-list');
    tbody.innerHTML = '';
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
    renderCustomerList(customers);

    const addContactButton = $('#add-customer-contact');
    if (!customers.length) {
        state.selectedCustomerId = null;
        addContactButton.disabled = true;
        if (showDetail) {
            renderCustomerPlaceholder();
        }
        return;
    }

    const selectedExists = customers.some(item => item.id === state.selectedCustomerId);
    if (!selectedExists) {
        state.selectedCustomerId = customers[0].id;
    }

    addContactButton.disabled = !state.selectedCustomerId || !showDetail;
    if (showDetail && state.selectedCustomerId) {
        await selectCustomer(state.selectedCustomerId);
    }
}

function renderCustomerList(items) {
    const tbody = $('#customer-list');
    tbody.innerHTML = '';
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
        setPillState(gmailScopeStatus, 'Read-only granted', 'success');
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
            setStatus(`Google connected: ${email}`, true);
        } catch (error) {
            console.error(error);
            await loadSettings();
            setStatus(getErrorMessage(error), false);
        }
    });

    $('#remove-google-account').addEventListener('click', async () => {
        try {
            await invoke('remove_google_account');
            await loadSettings();
            setStatus('Google account disconnected', true);
        } catch (error) {
            console.error(error);
            await loadSettings();
            setStatus(getErrorMessage(error), false);
        }
    });

    $('#test-gmail').addEventListener('click', async () => {
        try {
            const email = await invoke('test_gmail_connection');
            await loadSettings();
            setStatus(`Gmail ready: ${email}`, true);
        } catch (error) {
            console.error(error);
            await loadSettings();
            setStatus(getErrorMessage(error), false);
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
            setStatus('Invoice deleted', true);
        } catch (err) {
            console.error(err);
            setStatus('Failed to delete invoice', false);
        }
    });

    await loadSettings();
    await loadAppVersion();
    await loadDashboardYears();
    await loadDashboard();
    await loadInvoiceYears();
    await loadInvoices();
    await loadCustomers();

    await listen('invoice-updated', async event => {
        await loadDashboardYears();
        await loadDashboard();
        await loadInvoiceYears();
        await loadInvoices();
        await loadCustomers();
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
