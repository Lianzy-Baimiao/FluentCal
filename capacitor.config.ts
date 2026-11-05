import type { CapacitorConfig } from "@capacitor/cli";

const config: CapacitorConfig = {
  appId: "com.fluentcal.mobile",
  appName: "FluentCal",
  webDir: "dist-android",
  // Plugin arguments contain sync credentials and calendar content, including in debug APKs.
  loggingBehavior: "none",
  android: { backgroundColor: "#f3f3f3", minWebViewVersion: 120, adjustMarginsForEdgeToEdge: "auto" },
  server: { errorPath: 'webview-update.html' },
  plugins: {
    LocalNotifications: { smallIcon: "ic_stat_calendar", iconColor: "#0067C0" },
  },
};
export default config;
