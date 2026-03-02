/* Finance dashboard chart initialization — reads from global DATA object. */

const COLORS = [
  '#6366f1', '#8b5cf6', '#a78bfa', '#c084fc',
  '#e879f9', '#f472b6', '#fb7185', '#f87171',
  '#fbbf24', '#34d399', '#22d3ee', '#60a5fa',
];

function fmt(n) {
  return '$' + n.toLocaleString('en-CA', { minimumFractionDigits: 0, maximumFractionDigits: 0 });
}

function colorForRate(rate) {
  if (rate >= 0.20) return 'green';
  if (rate >= 0.10) return 'yellow';
  return 'red';
}

function colorForMonths(m) {
  if (m >= 6) return 'green';
  if (m >= 3) return 'yellow';
  return 'red';
}

function renderKPIs() {
  const k = DATA.kpis;
  document.getElementById('kpi-savings').textContent = (k.overall_savings_rate * 100).toFixed(1) + '%';
  document.getElementById('kpi-savings').className = 'kpi-value ' + colorForRate(k.overall_savings_rate);
  document.getElementById('kpi-spending').textContent = fmt(k.avg_monthly_spending);
  document.getElementById('kpi-balance').textContent = fmt(k.latest_balance);
  document.getElementById('kpi-emergency').textContent = k.emergency_fund_months.toFixed(1) + ' mo';
  document.getElementById('kpi-emergency').className = 'kpi-value ' + colorForMonths(k.emergency_fund_months);
  document.getElementById('date-range').textContent =
    `${DATA.date_range.start} to ${DATA.date_range.end}`;
}

function renderCashflowChart() {
  const last24 = DATA.monthly_cashflow.slice(-24);
  new Chart(document.getElementById('chart-cashflow'), {
    type: 'line',
    data: {
      labels: last24.map(m => m.month),
      datasets: [
        {
          label: 'Income',
          data: last24.map(m => m.income),
          borderColor: '#4ade80',
          backgroundColor: 'rgba(74,222,128,0.1)',
          tension: 0.3, fill: true,
        },
        {
          label: 'Expenses',
          data: last24.map(m => m.expenses),
          borderColor: '#f87171',
          backgroundColor: 'rgba(248,113,113,0.1)',
          tension: 0.3, fill: true,
        },
      ],
    },
    options: {
      responsive: true,
      plugins: { legend: { labels: { color: '#c0c0d0' } } },
      scales: {
        x: { ticks: { color: '#8888aa', maxRotation: 45 }, grid: { color: '#2a2a4a' } },
        y: { ticks: { color: '#8888aa', callback: v => fmt(v) }, grid: { color: '#2a2a4a' } },
      },
    },
  });
}

function renderCategoryChart() {
  const cats = DATA.category_spending;
  const labels = Object.keys(cats);
  const values = labels.map(k => cats[k].total);

  new Chart(document.getElementById('chart-categories'), {
    type: 'doughnut',
    data: {
      labels,
      datasets: [{ data: values, backgroundColor: COLORS.slice(0, labels.length) }],
    },
    options: {
      responsive: true,
      plugins: {
        legend: { position: 'right', labels: { color: '#c0c0d0', font: { size: 11 } } },
        tooltip: {
          callbacks: {
            label: ctx => {
              const total = values.reduce((a, b) => a + b, 0);
              const pct = ((ctx.raw / total) * 100).toFixed(1);
              return `${ctx.label}: ${fmt(ctx.raw)} (${pct}%)`;
            },
          },
        },
      },
    },
  });
}

function renderMonthlyBars() {
  const last12 = DATA.monthly_cashflow.slice(-12);
  new Chart(document.getElementById('chart-monthly'), {
    type: 'bar',
    data: {
      labels: last12.map(m => m.month),
      datasets: [
        {
          label: 'Expenses',
          data: last12.map(m => m.expenses),
          backgroundColor: 'rgba(99,102,241,0.7)',
          borderRadius: 4,
        },
        {
          label: 'Net Cash Flow',
          data: last12.map(m => m.net),
          type: 'line',
          borderColor: '#4ade80',
          borderWidth: 2,
          pointRadius: 3,
          tension: 0.3,
        },
      ],
    },
    options: {
      responsive: true,
      plugins: { legend: { labels: { color: '#c0c0d0' } } },
      scales: {
        x: { ticks: { color: '#8888aa' }, grid: { color: '#2a2a4a' } },
        y: { ticks: { color: '#8888aa', callback: v => fmt(v) }, grid: { color: '#2a2a4a' } },
      },
    },
  });
}

function renderMerchantsTable() {
  const tbody = document.getElementById('merchants-body');
  DATA.top_merchants.slice(0, 15).forEach(m => {
    const avg = m.count > 0 ? (m.total_spent / m.count) : 0;
    const tr = document.createElement('tr');
    tr.innerHTML = `
      <td>${m.description}</td>
      <td>${fmt(m.total_spent)}</td>
      <td>${m.count}</td>
      <td>${fmt(avg)}</td>
    `;
    tbody.appendChild(tr);
  });
}

/* E-Transfer drill-down: sortable table with expand/collapse */
function renderEtransferSection() {
  const data = DATA.etransfers || [];
  if (!data.length) return;

  const container = document.getElementById('etransfer-section');
  container.style.display = '';

  const totalSent = data.reduce((s, r) => s + r.total_sent, 0);
  const totalCount = data.reduce((s, r) => s + r.count, 0);
  document.getElementById('et-summary').textContent =
    `${fmt(totalSent)} sent across ${totalCount} transfers to ${data.length} recipients`;

  let sortKey = 'total_sent';
  let sortDir = -1; // descending
  const tbody = document.getElementById('etransfer-body');
  const headers = document.querySelectorAll('#etransfer-table th[data-sort]');

  function render() {
    const sorted = [...data].sort((a, b) => {
      const av = a[sortKey], bv = b[sortKey];
      if (typeof av === 'string') return sortDir * av.localeCompare(bv);
      return sortDir * (av - bv);
    });

    tbody.innerHTML = '';
    sorted.forEach(r => {
      const tr = document.createElement('tr');
      tr.innerHTML = `
        <td>${r.recipient}</td>
        <td>${fmt(r.total_sent)}</td>
        <td>${r.count}</td>
        <td>${r.last_date}</td>
        <td>${fmt(r.last_amount)}</td>
      `;
      tbody.appendChild(tr);
    });

    // Update sort indicators
    headers.forEach(th => {
      const arrow = th.dataset.sort === sortKey ? (sortDir > 0 ? ' ▲' : ' ▼') : '';
      th.textContent = th.dataset.label + arrow;
    });
  }

  headers.forEach(th => {
    th.dataset.label = th.textContent;
    th.style.cursor = 'pointer';
    th.addEventListener('click', () => {
      if (th.dataset.sort === sortKey) {
        sortDir *= -1;
      } else {
        sortKey = th.dataset.sort;
        sortDir = -1;
      }
      render();
    });
  });

  render();
}

document.addEventListener('DOMContentLoaded', () => {
  if (typeof DATA === 'undefined') {
    document.body.innerHTML = '<h1 style="color:#f87171;padding:40px">No DATA injected. Run dashboard.py first.</h1>';
    return;
  }
  renderKPIs();
  renderCashflowChart();
  renderCategoryChart();
  renderMonthlyBars();
  renderMerchantsTable();
  renderEtransferSection();
});
