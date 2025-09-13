class LLMDashboard {
    constructor() {
        this.ws = null;
        this.chart = null;
        this.isConnected = false;
        this.isGenerating = false;
        this.chartData = {
            labels: [],
            datasets: [
                {
                    label: 'Entries per Minute',
                    data: [],
                    borderColor: '#667eea',
                    backgroundColor: 'rgba(102, 126, 234, 0.1)',
                    tension: 0.3,
                    hidden: false
                },
                {
                    label: 'Total Unique Entries',
                    data: [],
                    borderColor: '#38a169',
                    backgroundColor: 'rgba(56, 161, 105, 0.1)',
                    tension: 0.3,
                    yAxisID: 'y1',
                    hidden: true
                }
            ]
        };
        
        this.init();
    }

    init() {
        this.setupEventListeners();
        this.initChart();
        this.connectWebSocket();
        this.addInitialLogEntry();
    }

    setupEventListeners() {
        // Form submission
        document.getElementById('topic-form').addEventListener('submit', (e) => {
            e.preventDefault();
            this.startGeneration();
        });

        // Stop button
        document.getElementById('stop-btn').addEventListener('click', () => {
            this.stopGeneration();
        });

        // Input validation
        document.getElementById('topic-input').addEventListener('input', (e) => {
            this.validateTopicInput(e.target);
        });

        // Producer count validation
        document.getElementById('producer-count').addEventListener('input', (e) => {
            this.validateProducerCount(e.target);
        });
        
        // Window events
        window.addEventListener('beforeunload', () => {
            if (this.ws) {
                this.ws.close();
            }
        });
    }

    initChart() {
        const ctx = document.getElementById('performance-chart').getContext('2d');
        
        this.chart = new Chart(ctx, {
            type: 'line',
            data: this.chartData,
            options: {
                responsive: true,
                maintainAspectRatio: false,
                interaction: {
                    intersect: false,
                    mode: 'index'
                },
                plugins: {
                    title: {
                        display: true,
                        text: 'System Performance Over Time'
                    },
                    legend: {
                        position: 'bottom'
                    }
                },
                scales: {
                    x: {
                        title: {
                            display: true,
                            text: 'Time'
                        },
                        ticks: {
                            maxTicksLimit: 10
                        }
                    },
                    y: {
                        type: 'linear',
                        display: true,
                        position: 'left',
                        title: {
                            display: true,
                            text: 'Entries per Minute'
                        },
                        beginAtZero: true
                    },
                    y1: {
                        type: 'linear',
                        display: true,
                        position: 'right',
                        title: {
                            display: true,
                            text: 'Total Unique Entries'
                        },
                        beginAtZero: true,
                        grid: {
                            drawOnChartArea: false,
                        },
                    }
                }
            }
        });
    }

    connectWebSocket() {
        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        const wsUrl = `${protocol}//${window.location.host}/ws`;
        
        this.addLogEntry('info', 'Connecting to system...');
        
        try {
            this.ws = new WebSocket(wsUrl);
            
            this.ws.onopen = () => {
                this.isConnected = true;
                this.updateConnectionStatus('connected', 'Connected to System');
                this.addLogEntry('success', 'WebSocket connection established');
                this.sendMessage({ type: 'RequestDashboard' });
            };
            
            this.ws.onmessage = (event) => {
                try {
                    const message = JSON.parse(event.data);
                    this.handleServerMessage(message);
                } catch (error) {
                    console.error('Failed to parse WebSocket message:', error);
                    this.addLogEntry('error', 'Failed to parse server message');
                }
            };
            
            this.ws.onclose = () => {
                this.isConnected = false;
                this.updateConnectionStatus('disconnected', 'Disconnected');
                this.addLogEntry('warning', 'WebSocket connection lost. Attempting to reconnect...');
                
                // Attempt to reconnect after delay
                setTimeout(() => {
                    if (!this.isConnected) {
                        this.connectWebSocket();
                    }
                }, 3000);
            };
            
            this.ws.onerror = (error) => {
                console.error('WebSocket error:', error);
                this.addLogEntry('error', 'WebSocket connection error');
                this.updateConnectionStatus('error', 'Connection Error');
            };
            
        } catch (error) {
            console.error('Failed to create WebSocket connection:', error);
            this.addLogEntry('error', 'Failed to create WebSocket connection');
            this.updateConnectionStatus('error', 'Connection Failed');
        }
    }
    
    handleServerMessage(message) {
        switch (message.type || message.constructor.name) {
            case 'MetricsUpdate':
                this.updateMetrics(message);
                break;
                
            case 'AttributeBatch':
                this.addNewAttributes(message);
                break;
                
            case 'SystemAlert':
                this.handleSystemAlert(message);
                break;
                
            case 'DashboardData':
                this.loadDashboardData(message);
                break;
                
            default:
                // Try to handle the message based on its content
                if (message.total_unique_entries !== undefined) {
                    this.updateMetrics(message);
                } else if (message.level && message.message) {
                    this.handleSystemAlert(message);
                } else if (Array.isArray(message)) {
                    this.addNewAttributes(message);
                }
                break;
        }
    }

    sendMessage(message) {
        if (this.ws && this.ws.readyState === WebSocket.OPEN) {
            this.ws.send(JSON.stringify(message));
        } else {
            this.addLogEntry('error', 'Cannot send message: WebSocket not connected');
        }
    }

    startGeneration() {
        const topicInput = document.getElementById('topic-input');
        const producerCountInput = document.getElementById('producer-count');
        const customPromptInput = document.getElementById('custom-prompt');
        
        const topic = topicInput.value.trim();
        const producerCount = parseInt(producerCountInput.value) || 5;
        const customPrompt = customPromptInput.value.trim() || null;
        
        if (!topic) {
            this.showValidationError(topicInput, 'Topic is required');
            return;
        }
        
        if (producerCount < 1 || producerCount > 10) {
            this.showValidationError(producerCountInput, 'Producer count must be between 1 and 10');
            return;
        }
        
        this.clearValidationErrors();
        
        const message = {
            type: 'StartTopic',
            topic,
            producer_count: producerCount,
            prompt: customPrompt
        };
        
        this.sendMessage(message);
        this.isGenerating = true;
        this.updateControlButtons();
        this.addLogEntry('info', `Starting generation for topic: "${topic}" with ${producerCount} producers`);
    }

    stopGeneration() {
        const message = { type: 'StopGeneration' };
        this.sendMessage(message);
        this.isGenerating = false;
        this.updateControlButtons();
        this.addLogEntry('info', 'Stopping generation...');
    }

    updateMetrics(metrics) {
        // Update metric values
        this.updateMetricDisplay('total-unique', metrics.total_unique_entries || 0);
        this.updateMetricDisplay('entries-per-minute', (metrics.entries_per_minute || 0).toFixed(1));
        this.updateMetricDisplay('active-producers', metrics.active_producers || 0);
        this.updateMetricDisplay('system-uptime', this.formatUptime(metrics.uptime_seconds || 0));
        
        // Update current topic
        const topicElement = document.getElementById('current-topic');
        topicElement.textContent = metrics.current_topic || 'None';
        
        // Update chart
        this.updateChart(metrics);
        
        // Update provider performance
        this.updateProviderPerformance(metrics.per_llm_performance || {});
    }

    updateMetricDisplay(elementId, value) {
        const element = document.getElementById(elementId);
        if (element && element.textContent !== value.toString()) {
            element.textContent = value;
            element.closest('.metric-card')?.classList.add('updated');
            setTimeout(() => {
                element.closest('.metric-card')?.classList.remove('updated');
            }, 500);
        }
    }

    updateChart(metrics) {
        const now = new Date().toLocaleTimeString();
        
        // Add new data point
        this.chartData.labels.push(now);
        this.chartData.datasets[0].data.push(metrics.entries_per_minute || 0);
        this.chartData.datasets[1].data.push(metrics.total_unique_entries || 0);
        
        // Keep only last 20 data points
        const maxPoints = 20;
        if (this.chartData.labels.length > maxPoints) {
            this.chartData.labels.shift();
            this.chartData.datasets.forEach(dataset => dataset.data.shift());
        }
        
        this.chart.update('none'); // No animation for better performance
    }

    updateProviderPerformance(performanceData) {
        const container = document.getElementById('provider-performance');
        container.innerHTML = '';
        
        for (const [provider, stats] of Object.entries(performanceData)) {
            const providerCard = document.createElement('div');
            providerCard.className = `provider-card ${provider.toLowerCase()}`;
            
            const healthClass = this.getProviderHealthClass(stats);
            providerCard.classList.add(healthClass);
            
            providerCard.innerHTML = `
                <div class="provider-name">${provider}</div>
                <div class="provider-stats">
                    <div class="provider-stat">
                        <span class="provider-stat-label">Unique Generated:</span>
                        <span class="provider-stat-value">${stats.unique_generated || 0}</span>
                    </div>
                    <div class="provider-stat">
                        <span class="provider-stat-label">Success Rate:</span>
                        <span class="provider-stat-value">${((stats.success_rate || 0) * 100).toFixed(1)}%</span>
                    </div>
                    <div class="provider-stat">
                        <span class="provider-stat-label">Avg Response:</span>
                        <span class="provider-stat-value">${stats.average_response_time_ms || 0}ms</span>
                    </div>
                    <div class="provider-stat">
                        <span class="provider-stat-label">Efficiency:</span>
                        <span class="provider-stat-value">${((stats.uniqueness_ratio || 0) * 100).toFixed(1)}%</span>
                    </div>
                </div>
            `;
            
            container.appendChild(providerCard);
        }
    }

    getProviderHealthClass(stats) {
        const successRate = stats.success_rate || 0;
        const avgResponseTime = stats.average_response_time_ms || 0;
        
        if (successRate > 0.8 && avgResponseTime < 5000) return 'healthy';
        if (successRate > 0.5 && avgResponseTime < 10000) return 'degraded';
        return 'unhealthy';
    }

    addNewAttributes(attributes) {
        const container = document.getElementById('attributes-container');
        
        // Remove "no attributes" message if present
        const noAttributesMsg = container.querySelector('.no-attributes');
        if (noAttributesMsg) {
            noAttributesMsg.remove();
        }
        
        // Add new attributes to the top
        for (const attr of attributes.reverse()) {
            const attributeElement = document.createElement('div');
            attributeElement.className = 'attribute-item new';
            
            const timeStr = new Date(attr.timestamp * 1000).toLocaleTimeString();
            
            attributeElement.innerHTML = `
                <div class="attribute-content">${this.escapeHtml(attr.content)}</div>
                <div class="attribute-meta">
                    <div class="attribute-provider">${attr.llm_provider || 'unknown'}</div>
                    <div class="attribute-time">${timeStr}</div>
                </div>
            `;
            
            container.insertBefore(attributeElement, container.firstChild);
        }
        
        // Remove animation class after animation completes
        setTimeout(() => {
            container.querySelectorAll('.attribute-item.new').forEach(el => {
                el.classList.remove('new');
            });
        }, 300);
        
        // Keep only latest 50 attributes for performance
        const items = container.querySelectorAll('.attribute-item');
        if (items.length > 50) {
            for (let i = 50; i < items.length; i++) {
                items[i].remove();
            }
        }
        
        // Update recent count
        const recentCountElement = document.getElementById('recent-count');
        recentCountElement.textContent = Math.min(items.length, 50);
    }

    handleSystemAlert(alert) {
        const levelClass = alert.level ? `log-${alert.level.toLowerCase()}` : 'log-info';
        const timeStr = new Date(alert.timestamp * 1000).toLocaleTimeString();
        const message = alert.message || 'System notification';
        
        this.addLogEntry(alert.level?.toLowerCase() || 'info', message, timeStr);
    }

    loadDashboardData(data) {
        if (data.metrics) {
            this.updateMetrics(data.metrics);
        }
        
        if (data.recent_attributes) {
            this.addNewAttributes(data.recent_attributes);
        }
        
        if (data.system_health) {
            this.updateSystemHealth(data.system_health);
        }
    }

    updateConnectionStatus(status, text) {
        const statusDot = document.getElementById('status-dot');
        const statusText = document.getElementById('status-text');
        
        statusDot.className = `status-dot ${status}`;
        statusText.textContent = text;
    }

    updateControlButtons() {
        const startBtn = document.getElementById('start-btn');
        const stopBtn = document.getElementById('stop-btn');
        
        if (this.isGenerating) {
            startBtn.disabled = true;
            stopBtn.disabled = false;
            startBtn.textContent = 'Generating...';
        } else {
            startBtn.disabled = false;
            stopBtn.disabled = true;
            startBtn.textContent = 'Start Generation';
        }
    }

    validateTopicInput(input) {
        const topic = input.value.trim();
        const formGroup = input.closest('.form-group');
        
        formGroup.classList.remove('error', 'success');
        
        if (topic.length < 3) {
            formGroup.classList.add('error');
            return false;
        } else if (topic.length > 2) {
            formGroup.classList.add('success');
            return true;
        }
    }

    validateProducerCount(input) {
        const count = parseInt(input.value);
        const formGroup = input.closest('.form-group');
        
        formGroup.classList.remove('error', 'success');
        
        if (count < 1 || count > 10 || isNaN(count)) {
            formGroup.classList.add('error');
            return false;
        } else {
            formGroup.classList.add('success');
            return true;
        }
    }

    showValidationError(input, message) {
        const formGroup = input.closest('.form-group');
        formGroup.classList.add('error');
        this.addLogEntry('error', message);
        input.focus();
    }

    clearValidationErrors() {
        document.querySelectorAll('.form-group.error').forEach(group => {
            group.classList.remove('error');
        });
    }

    addLogEntry(level, message, timeStr = null) {
        const container = document.getElementById('log-container');
        const logEntry = document.createElement('div');
        logEntry.className = `log-entry log-${level}`;
        
        const time = timeStr || new Date().toLocaleTimeString();
        
        logEntry.innerHTML = `
            <span class="log-time">${time}</span>
            <span class="log-message">${this.escapeHtml(message)}</span>
        `;
        
        // Add to top of log container
        container.insertBefore(logEntry, container.firstChild);
        
        // Keep only latest 100 log entries
        const entries = container.querySelectorAll('.log-entry');
        if (entries.length > 100) {
            for (let i = 100; i < entries.length; i++) {
                entries[i].remove();
            }
        }
    }

    addInitialLogEntry() {
        this.addLogEntry('info', 'Dashboard initialized. Waiting for connection...');
    }

    formatUptime(seconds) {
        const hours = Math.floor(seconds / 3600);
        const minutes = Math.floor((seconds % 3600) / 60);
        const secs = seconds % 60;
        
        if (hours > 0) {
            return `${hours}h ${minutes}m`;
        } else if (minutes > 0) {
            return `${minutes}m ${secs}s`;
        } else {
            return `${secs}s`;
        }
    }

    updateSystemHealth(health) {
        // Update connection status based on system health
        if (health.orchestrator_connected) {
            this.updateConnectionStatus('connected', 'System Healthy');
        } else {
            this.updateConnectionStatus('error', 'System Issues');
        }
    }

    // Chart control methods
    toggleChartData(datasetKey) {
        let datasetIndex;
        switch (datasetKey) {
            case 'rate':
                datasetIndex = 0;
                break;
            case 'unique':
                datasetIndex = 1;
                break;
            default:
                return;
        }
        
        const dataset = this.chart.data.datasets[datasetIndex];
        dataset.hidden = !dataset.hidden;
        this.chart.update();
        
        this.addLogEntry('info', `${dataset.hidden ? 'Hidden' : 'Shown'} ${dataset.label} chart data`);
    }

    clearChart() {
        this.chartData.labels = [];
        this.chartData.datasets.forEach(dataset => {
            dataset.data = [];
        });
        this.chart.update();
        this.addLogEntry('info', 'Performance chart cleared');
    }

    clearLogs() {
        const container = document.getElementById('log-container');
        container.innerHTML = '';
        this.addLogEntry('info', 'Log history cleared');
    }

    escapeHtml(text) {
        const div = document.createElement('div');
        div.textContent = text;
        return div.innerHTML;
    }
}

// Initialize dashboard when DOM is loaded
document.addEventListener('DOMContentLoaded', () => {
    window.dashboard = new LLMDashboard();
});

// Expose dashboard instance globally for console access
window.addEventListener('load', () => {
    console.log('LLM Orchestration Dashboard loaded');
    console.log('Access dashboard via: window.dashboard');
});