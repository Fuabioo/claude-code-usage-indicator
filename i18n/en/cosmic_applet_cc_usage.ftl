# Application metadata
app-name = Claude Code Usage
app-description = Monitor Claude Code weekly and hourly usage budget

# Panel view labels
weekly-budget = Weekly
hourly-session = Session
daily-budget-pace = Daily Pace

# Pace status indicators
pace-on-track = On Track
pace-caution = Caution
pace-over-budget = Over Budget

# Time and budget display
resets-in = Resets in {$time}
todays-ceiling = Today's ceiling
todays-ceiling-detail = {$label}: {$ceiling}% ({$weekday}, day {$index}/{$total})
consumed = Consumed
consumed-detail = {$label}: {$consumed}% of {$ceiling}% ceiling
remaining-today = Remaining today
over-by = Over by

# Staleness indicators
last-updated = Last updated: {$time} ago
data-may-be-stale = Data may be stale
loading-usage-data = Loading usage data...

# Error messages
error-credentials-not-found = Credentials not found at configured path
error-unable-to-fetch = Unable to fetch usage data
# error-token-expired = Token expired or invalid
error-rate-limited = Rate limited by API
error-network = Network error: {$details}
error-parse = Failed to parse API response
error-unauthorized = Unauthorized - token may be expired

# Unused keys (reserved for future config UI):
# config-poll-interval = Poll interval (seconds)
# config-work-days = Work days per week
# config-daily-budget = Daily budget (%)
# config-credentials-path = Credentials file path

# Unused tooltip keys (reserved for potential panel icon tooltips):
# tooltip-weekly-usage = Weekly usage: {$percentage}% of 1,000 interactions
# tooltip-hourly-session = Current hour: {$percentage}% of 100 interactions
# tooltip-click-for-details = Click for detailed budget breakdown
