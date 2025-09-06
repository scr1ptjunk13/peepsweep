import { useState, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { 
  Settings as SettingsIcon,
  TrendingUp,
  Palette,
  Bell,
  Shield,
  Code,
  User,
  Sliders,
  Volume2,
  Eye,
  Clock,
  Zap,
  CheckCircle,
  AlertTriangle,
  Download,
  Upload,
  RefreshCw,
  Save,
  X,
  Info
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { Slider } from "@/components/ui/slider";
import Header from "@/components/header";

type SettingsCategory = 'trading' | 'interface' | 'notifications' | 'security' | 'api' | 'account';

interface Settings {
  // Trading Preferences
  slippageTolerance: number;
  gasPriceStrategy: 'slow' | 'standard' | 'fast' | 'custom';
  customGasPrice: string;
  autoRefreshQuotes: boolean;
  refreshInterval: number;
  mevProtection: boolean;
  transactionDeadline: number;
  preferredDexs: string[];
  
  // Interface & Display
  theme: 'dark' | 'darker' | 'midnight';
  accentColor: string;
  animationSpeed: number;
  soundEffects: boolean;
  soundVolume: number;
  numberFormat: 'decimals' | 'scientific' | 'compact';
  defaultTimeframe: '1h' | '4h' | '1d' | '7d';
  
  // Notifications
  browserNotifications: boolean;
  priceAlerts: boolean;
  transactionAlerts: boolean;
  emailAlerts: boolean;
  pushNotifications: boolean;
  discordWebhook: string;
  telegramBot: string;
  alertThreshold: number;
  
  // Security & Privacy
  autoLockWallet: number;
  requireConfirmation: boolean;
  dataCollection: boolean;
  twoFactorAuth: boolean;
  biometricAuth: boolean;
  
  // API & Advanced
  apiKeys: string[];
  customRpcUrl: string;
  developerMode: boolean;
  debugLogging: boolean;
  
  // Account
  username: string;
  email: string;
  language: string;
}

const initialSettings: Settings = {
  slippageTolerance: 0.5,
  gasPriceStrategy: 'standard',
  customGasPrice: '20',
  autoRefreshQuotes: true,
  refreshInterval: 10,
  mevProtection: true,
  transactionDeadline: 10,
  preferredDexs: ['uniswap', 'sushiswap'],
  
  theme: 'dark',
  accentColor: '#39FF14',
  animationSpeed: 1,
  soundEffects: true,
  soundVolume: 50,
  numberFormat: 'decimals',
  defaultTimeframe: '1d',
  
  browserNotifications: true,
  priceAlerts: true,
  transactionAlerts: true,
  emailAlerts: false,
  pushNotifications: true,
  discordWebhook: '',
  telegramBot: '',
  alertThreshold: 5,
  
  autoLockWallet: 30,
  requireConfirmation: true,
  dataCollection: true,
  twoFactorAuth: false,
  biometricAuth: false,
  
  apiKeys: [],
  customRpcUrl: '',
  developerMode: false,
  debugLogging: false,
  
  username: '',
  email: '',
  language: 'en'
};

const settingsCategories = [
  { id: 'trading' as SettingsCategory, name: 'Trading Preferences', icon: TrendingUp },
  { id: 'interface' as SettingsCategory, name: 'Interface & Display', icon: Palette },
  { id: 'notifications' as SettingsCategory, name: 'Notifications', icon: Bell },
  { id: 'security' as SettingsCategory, name: 'Security & Privacy', icon: Shield },
  { id: 'api' as SettingsCategory, name: 'API & Advanced', icon: Code },
  { id: 'account' as SettingsCategory, name: 'Account Management', icon: User }
];

const availableDexs = [
  { id: 'uniswap', name: 'Uniswap V3', logo: 'bg-gradient-to-br from-pink-500 to-purple-600' },
  { id: 'sushiswap', name: 'SushiSwap', logo: 'bg-gradient-to-br from-blue-500 to-teal-500' },
  { id: 'curve', name: 'Curve Finance', logo: 'bg-gradient-to-br from-yellow-500 to-orange-500' },
  { id: 'balancer', name: 'Balancer', logo: 'bg-gradient-to-br from-gray-500 to-gray-700' }
];

const accentColors = [
  '#39FF14', // Electric Lime
  '#00D4FF', // Nuclear Blue
  '#FFFF00', // Lightning Yellow
  '#00FF88', // Velocity Green
  '#FF1493', // Deep Pink
  '#FF4500', // Orange Red
];

export default function Settings() {
  const [activeCategory, setActiveCategory] = useState<SettingsCategory>('trading');
  const [settings, setSettings] = useState<Settings>(initialSettings);
  const [originalSettings, setOriginalSettings] = useState<Settings>(initialSettings);
  const [hasUnsavedChanges, setHasUnsavedChanges] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [showResetDialog, setShowResetDialog] = useState(false);

  // Check for unsaved changes
  useEffect(() => {
    const hasChanges = JSON.stringify(settings) !== JSON.stringify(originalSettings);
    setHasUnsavedChanges(hasChanges);
  }, [settings, originalSettings]);

  const updateSetting = <K extends keyof Settings>(key: K, value: Settings[K]) => {
    setSettings(prev => ({ ...prev, [key]: value }));
  };

  const handleSave = async () => {
    setIsSaving(true);
    // Simulate API call
    await new Promise(resolve => setTimeout(resolve, 1000));
    setOriginalSettings(settings);
    setIsSaving(false);
  };

  const handleReset = () => {
    setSettings(originalSettings);
    setShowResetDialog(false);
  };

  const handleResetToDefaults = () => {
    setSettings(initialSettings);
    setShowResetDialog(false);
  };

  const SettingItem = ({ 
    title, 
    description, 
    children, 
    tooltip 
  }: { 
    title: string; 
    description?: string; 
    children: React.ReactNode;
    tooltip?: string;
  }) => (
    <div className="flex items-center justify-between p-4 bg-black/20 border border-gray-800 hover:border-gray-700 transition-colors duration-100">
      <div className="flex-1 mr-4">
        <div className="flex items-center space-x-2">
          <h4 className="text-sm font-medium">{title}</h4>
          {tooltip && (
            <div className="group relative">
              <Info className="w-3 h-3 text-gray-500 hover:text-electric-lime cursor-help" />
              <div className="absolute left-0 bottom-full mb-2 hidden group-hover:block w-64 p-2 bg-gray-900 border border-electric-lime/30 text-xs z-50">
                {tooltip}
              </div>
            </div>
          )}
        </div>
        {description && (
          <p className="text-xs text-gray-400 mt-1 italic-forward">{description}</p>
        )}
      </div>
      <div className="flex-shrink-0">
        {children}
      </div>
    </div>
  );

  const renderTradingSettings = () => (
    <div className="space-y-4">
      <SettingItem
        title="Slippage Tolerance"
        description="Maximum price movement you'll accept"
        tooltip="Higher slippage allows trades in volatile markets but may result in worse prices"
      >
        <div className="flex items-center space-x-4 w-48">
          <Slider
            value={[settings.slippageTolerance]}
            onValueChange={(value) => updateSetting('slippageTolerance', value[0])}
            max={5}
            min={0.1}
            step={0.1}
            className="flex-1"
          />
          <span className="text-sm font-mono w-12 text-right">{settings.slippageTolerance}%</span>
        </div>
      </SettingItem>

      <SettingItem
        title="Gas Price Strategy"
        description="How fast you want your transactions confirmed"
      >
        <div className="space-y-2">
          {['slow', 'standard', 'fast', 'custom'].map((strategy) => (
            <label key={strategy} className="flex items-center space-x-2 cursor-pointer">
              <input
                type="radio"
                name="gasStrategy"
                value={strategy}
                checked={settings.gasPriceStrategy === strategy}
                onChange={(e) => updateSetting('gasPriceStrategy', e.target.value as any)}
                className="text-electric-lime focus:ring-electric-lime"
              />
              <span className="text-sm capitalize">{strategy}</span>
              {strategy === 'custom' && settings.gasPriceStrategy === 'custom' && (
                <Input
                  type="number"
                  value={settings.customGasPrice}
                  onChange={(e) => updateSetting('customGasPrice', e.target.value)}
                  className="w-20 h-6 text-xs"
                  placeholder="GWEI"
                />
              )}
            </label>
          ))}
        </div>
      </SettingItem>

      <SettingItem
        title="Auto-refresh Quotes"
        description="Automatically update swap quotes"
      >
        <div className="flex items-center space-x-4">
          <Switch
            checked={settings.autoRefreshQuotes}
            onCheckedChange={(checked) => updateSetting('autoRefreshQuotes', checked)}
          />
          {settings.autoRefreshQuotes && (
            <select 
              value={settings.refreshInterval}
              onChange={(e) => updateSetting('refreshInterval', Number(e.target.value))}
              className="bg-gray-800 border border-gray-600 px-2 py-1 text-xs"
            >
              <option value={5}>5s</option>
              <option value={10}>10s</option>
              <option value={15}>15s</option>
              <option value={30}>30s</option>
            </select>
          )}
        </div>
      </SettingItem>

      <SettingItem
        title="MEV Protection"
        description="Protect against frontrunning attacks"
        tooltip="MEV protection routes trades through private mempools to prevent sandwich attacks"
      >
        <Switch
          checked={settings.mevProtection}
          onCheckedChange={(checked) => updateSetting('mevProtection', checked)}
        />
      </SettingItem>

      <SettingItem
        title="Transaction Deadline"
        description="Cancel transaction if not mined within time limit"
      >
        <div className="flex items-center space-x-2">
          <Input
            type="number"
            value={settings.transactionDeadline}
            onChange={(e) => updateSetting('transactionDeadline', Number(e.target.value))}
            className="w-20 h-8 text-sm"
            min={1}
            max={60}
          />
          <span className="text-sm text-gray-400">minutes</span>
        </div>
      </SettingItem>

      <SettingItem
        title="Preferred DEXs"
        description="Prioritize routing through selected exchanges"
      >
        <div className="grid grid-cols-2 gap-2">
          {availableDexs.map((dex) => (
            <label key={dex.id} className="flex items-center space-x-2 cursor-pointer">
              <input
                type="checkbox"
                checked={settings.preferredDexs.includes(dex.id)}
                onChange={(e) => {
                  if (e.target.checked) {
                    updateSetting('preferredDexs', [...settings.preferredDexs, dex.id]);
                  } else {
                    updateSetting('preferredDexs', settings.preferredDexs.filter(id => id !== dex.id));
                  }
                }}
                className="text-electric-lime focus:ring-electric-lime"
              />
              <div className={`w-4 h-4 ${dex.logo} rounded-full`} />
              <span className="text-sm">{dex.name}</span>
            </label>
          ))}
        </div>
      </SettingItem>
    </div>
  );

  const renderInterfaceSettings = () => (
    <div className="space-y-4">
      <SettingItem
        title="Accent Color"
        description="Customize the primary theme color"
      >
        <div className="flex items-center space-x-2">
          {accentColors.map((color) => (
            <motion.button
              key={color}
              className={`w-8 h-8 rounded-full border-2 ${
                settings.accentColor === color ? 'border-white' : 'border-gray-600'
              }`}
              style={{ backgroundColor: color }}
              onClick={() => updateSetting('accentColor', color)}
              whileHover={{ scale: 1.1 }}
              whileTap={{ scale: 0.9 }}
            />
          ))}
        </div>
      </SettingItem>

      <SettingItem
        title="Animation Speed"
        description="Control interface animation intensity"
      >
        <div className="flex items-center space-x-4 w-48">
          <span className="text-xs text-gray-400">Reduced</span>
          <Slider
            value={[settings.animationSpeed]}
            onValueChange={(value) => updateSetting('animationSpeed', value[0])}
            max={2}
            min={0.5}
            step={0.1}
            className="flex-1"
          />
          <span className="text-xs text-gray-400">Enhanced</span>
        </div>
      </SettingItem>

      <SettingItem
        title="Sound Effects"
        description="Enable interface sounds and notifications"
      >
        <div className="flex items-center space-x-4">
          <Switch
            checked={settings.soundEffects}
            onCheckedChange={(checked) => updateSetting('soundEffects', checked)}
          />
          {settings.soundEffects && (
            <div className="flex items-center space-x-2 w-32">
              <Volume2 className="w-4 h-4 text-gray-400" />
              <Slider
                value={[settings.soundVolume]}
                onValueChange={(value) => updateSetting('soundVolume', value[0])}
                max={100}
                min={0}
                step={5}
                className="flex-1"
              />
            </div>
          )}
        </div>
      </SettingItem>

      <SettingItem
        title="Number Format"
        description="How numbers are displayed throughout the app"
      >
        <select 
          value={settings.numberFormat}
          onChange={(e) => updateSetting('numberFormat', e.target.value as any)}
          className="bg-gray-800 border border-gray-600 px-3 py-2 text-sm"
        >
          <option value="decimals">Decimals (1,234.56)</option>
          <option value="scientific">Scientific (1.23e6)</option>
          <option value="compact">Compact (1.23K)</option>
        </select>
      </SettingItem>

      <SettingItem
        title="Default Chart Timeframe"
        description="Default time range for price charts"
      >
        <div className="flex space-x-2">
          {['1h', '4h', '1d', '7d'].map((timeframe) => (
            <motion.button
              key={timeframe}
              className={`px-3 py-1 text-sm transition-all duration-100 ${
                settings.defaultTimeframe === timeframe
                  ? 'bg-electric-lime text-black font-bold'
                  : 'bg-gray-800 text-gray-300 hover:bg-gray-700'
              }`}
              onClick={() => updateSetting('defaultTimeframe', timeframe as any)}
              whileHover={{ scale: 1.05 }}
              whileTap={{ scale: 0.95 }}
            >
              {timeframe}
            </motion.button>
          ))}
        </div>
      </SettingItem>
    </div>
  );

  const renderNotificationSettings = () => (
    <div className="space-y-4">
      <SettingItem
        title="Browser Notifications"
        description="Show desktop notifications for important events"
      >
        <Switch
          checked={settings.browserNotifications}
          onCheckedChange={(checked) => updateSetting('browserNotifications', checked)}
        />
      </SettingItem>

      <SettingItem
        title="Price Alerts"
        description="Get notified when token prices change significantly"
      >
        <Switch
          checked={settings.priceAlerts}
          onCheckedChange={(checked) => updateSetting('priceAlerts', checked)}
        />
      </SettingItem>

      <SettingItem
        title="Transaction Alerts"
        description="Notifications for transaction confirmations and failures"
      >
        <Switch
          checked={settings.transactionAlerts}
          onCheckedChange={(checked) => updateSetting('transactionAlerts', checked)}
        />
      </SettingItem>

      <SettingItem
        title="Email Notifications"
        description="Send important alerts to your email"
      >
        <Switch
          checked={settings.emailAlerts}
          onCheckedChange={(checked) => updateSetting('emailAlerts', checked)}
        />
      </SettingItem>

      <SettingItem
        title="Alert Threshold"
        description="Minimum price change percentage to trigger alerts"
      >
        <div className="flex items-center space-x-4 w-48">
          <Slider
            value={[settings.alertThreshold]}
            onValueChange={(value) => updateSetting('alertThreshold', value[0])}
            max={20}
            min={1}
            step={0.5}
            className="flex-1"
          />
          <span className="text-sm font-mono w-12 text-right">{settings.alertThreshold}%</span>
        </div>
      </SettingItem>

      <SettingItem
        title="Discord Webhook"
        description="Send alerts to your Discord channel"
      >
        <Input
          type="text"
          value={settings.discordWebhook}
          onChange={(e) => updateSetting('discordWebhook', e.target.value)}
          placeholder="https://discord.com/api/webhooks/..."
          className="w-64"
        />
      </SettingItem>
    </div>
  );

  const renderSecuritySettings = () => (
    <div className="space-y-4">
      <SettingItem
        title="Auto-lock Wallet"
        description="Automatically lock wallet after inactivity"
      >
        <select 
          value={settings.autoLockWallet}
          onChange={(e) => updateSetting('autoLockWallet', Number(e.target.value))}
          className="bg-gray-800 border border-gray-600 px-3 py-2 text-sm"
        >
          <option value={0}>Never</option>
          <option value={5}>5 minutes</option>
          <option value={15}>15 minutes</option>
          <option value={30}>30 minutes</option>
          <option value={60}>1 hour</option>
        </select>
      </SettingItem>

      <SettingItem
        title="Transaction Confirmation"
        description="Require explicit confirmation for all transactions"
      >
        <Switch
          checked={settings.requireConfirmation}
          onCheckedChange={(checked) => updateSetting('requireConfirmation', checked)}
        />
      </SettingItem>

      <SettingItem
        title="Two-Factor Authentication"
        description="Add an extra layer of security to your account"
      >
        <div className="flex items-center space-x-2">
          <Switch
            checked={settings.twoFactorAuth}
            onCheckedChange={(checked) => updateSetting('twoFactorAuth', checked)}
          />
          {settings.twoFactorAuth && (
            <Button className="btn-secondary text-xs" data-testid="setup-2fa">
              Setup 2FA
            </Button>
          )}
        </div>
      </SettingItem>

      <SettingItem
        title="Data Collection"
        description="Allow anonymous usage data collection to improve the service"
      >
        <Switch
          checked={settings.dataCollection}
          onCheckedChange={(checked) => updateSetting('dataCollection', checked)}
        />
      </SettingItem>

      <SettingItem
        title="Biometric Authentication"
        description="Use fingerprint or face ID when available"
      >
        <Switch
          checked={settings.biometricAuth}
          onCheckedChange={(checked) => updateSetting('biometricAuth', checked)}
        />
      </SettingItem>
    </div>
  );

  const renderApiSettings = () => (
    <div className="space-y-4">
      <SettingItem
        title="Developer Mode"
        description="Enable advanced features and debugging options"
      >
        <Switch
          checked={settings.developerMode}
          onCheckedChange={(checked) => updateSetting('developerMode', checked)}
        />
      </SettingItem>

      <SettingItem
        title="Debug Logging"
        description="Enable detailed console logging for troubleshooting"
      >
        <Switch
          checked={settings.debugLogging}
          onCheckedChange={(checked) => updateSetting('debugLogging', checked)}
        />
      </SettingItem>

      <SettingItem
        title="Custom RPC Endpoint"
        description="Use a custom Ethereum RPC endpoint"
      >
        <Input
          type="text"
          value={settings.customRpcUrl}
          onChange={(e) => updateSetting('customRpcUrl', e.target.value)}
          placeholder="https://mainnet.infura.io/v3/..."
          className="w-64"
        />
      </SettingItem>

      <SettingItem
        title="API Key Management"
        description="Generate and manage API keys for external access"
      >
        <div className="space-y-2">
          <Button className="btn-secondary text-xs" data-testid="generate-api-key">
            Generate New Key
          </Button>
          {settings.apiKeys.length > 0 && (
            <div className="text-xs text-gray-400">
              {settings.apiKeys.length} active key(s)
            </div>
          )}
        </div>
      </SettingItem>

      <SettingItem
        title="Export Settings"
        description="Download your current settings as a backup"
      >
        <Button className="btn-accent text-xs" data-testid="export-settings">
          <Download className="w-3 h-3 mr-1" />
          Export
        </Button>
      </SettingItem>
    </div>
  );

  const renderAccountSettings = () => (
    <div className="space-y-4">
      <SettingItem
        title="Username"
        description="Your display name (optional)"
      >
        <Input
          type="text"
          value={settings.username}
          onChange={(e) => updateSetting('username', e.target.value)}
          placeholder="Enter username"
          className="w-48"
        />
      </SettingItem>

      <SettingItem
        title="Email Address"
        description="For notifications and account recovery"
      >
        <Input
          type="email"
          value={settings.email}
          onChange={(e) => updateSetting('email', e.target.value)}
          placeholder="Enter email"
          className="w-48"
        />
      </SettingItem>

      <SettingItem
        title="Language"
        description="Interface language preference"
      >
        <select 
          value={settings.language}
          onChange={(e) => updateSetting('language', e.target.value)}
          className="bg-gray-800 border border-gray-600 px-3 py-2 text-sm"
        >
          <option value="en">English</option>
          <option value="es">Español</option>
          <option value="fr">Français</option>
          <option value="de">Deutsch</option>
          <option value="ja">日本語</option>
          <option value="ko">한국어</option>
        </select>
      </SettingItem>

      <SettingItem
        title="Account Data"
        description="Export or delete your account data"
      >
        <div className="flex space-x-2">
          <Button className="btn-secondary text-xs" data-testid="export-account-data">
            <Download className="w-3 h-3 mr-1" />
            Export Data
          </Button>
          <Button className="bg-red-600 hover:bg-red-700 text-white px-3 py-1 text-xs" data-testid="delete-account">
            Delete Account
          </Button>
        </div>
      </SettingItem>
    </div>
  );

  const renderSettingsContent = () => {
    switch (activeCategory) {
      case 'trading':
        return renderTradingSettings();
      case 'interface':
        return renderInterfaceSettings();
      case 'notifications':
        return renderNotificationSettings();
      case 'security':
        return renderSecuritySettings();
      case 'api':
        return renderApiSettings();
      case 'account':
        return renderAccountSettings();
      default:
        return null;
    }
  };

  return (
    <div className="relative z-10">
      <Header />
      
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
        {/* Page Header */}
        <motion.div 
          className="flex items-center justify-between mb-8"
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.3 }}
        >
          <div>
            <h1 className="text-3xl font-bold italic-forward mb-2">Settings</h1>
            <p className="text-gray-400">Customize your HyperDEX experience</p>
          </div>
          
          <div className="flex items-center space-x-4">
            {/* Unsaved Changes Indicator */}
            <AnimatePresence>
              {hasUnsavedChanges && (
                <motion.div
                  className="flex items-center space-x-2 text-lightning-yellow text-sm"
                  initial={{ opacity: 0, x: 20 }}
                  animate={{ opacity: 1, x: 0 }}
                  exit={{ opacity: 0, x: 20 }}
                >
                  <AlertTriangle className="w-4 h-4" />
                  <span>Unsaved changes</span>
                </motion.div>
              )}
            </AnimatePresence>
            
            {/* Action Buttons */}
            <div className="flex space-x-2">
              <Button
                onClick={() => setShowResetDialog(true)}
                className="btn-secondary"
                disabled={!hasUnsavedChanges}
                data-testid="reset-settings"
              >
                <RefreshCw className="w-4 h-4 mr-2" />
                Reset
              </Button>
              
              <Button
                onClick={handleSave}
                className="btn-lightning"
                disabled={!hasUnsavedChanges || isSaving}
                data-testid="save-settings"
              >
                {isSaving ? (
                  <>
                    <RefreshCw className="w-4 h-4 mr-2 animate-spin" />
                    Saving...
                  </>
                ) : (
                  <>
                    <Save className="w-4 h-4 mr-2" />
                    Save Changes
                  </>
                )}
              </Button>
            </div>
          </div>
        </motion.div>

        <div className="grid grid-cols-1 lg:grid-cols-12 gap-8">
          {/* Left Sidebar - Categories */}
          <motion.div 
            className="lg:col-span-3"
            initial={{ opacity: 0, x: -20 }}
            animate={{ opacity: 1, x: 0 }}
            transition={{ duration: 0.3 }}
          >
            <div className="bg-gray-900/80 border border-gray-700 backdrop-blur-sm p-4">
              <h3 className="text-lg font-bold italic-forward mb-4">Categories</h3>
              <nav className="space-y-2">
                {settingsCategories.map((category) => {
                  const Icon = category.icon;
                  return (
                    <motion.button
                      key={category.id}
                      className={`w-full flex items-center space-x-3 px-3 py-3 text-left transition-all duration-100 ${
                        activeCategory === category.id
                          ? 'bg-electric-lime/20 border border-electric-lime/50 text-electric-lime'
                          : 'hover:bg-gray-800 text-gray-300 hover:text-white border border-transparent'
                      }`}
                      onClick={() => setActiveCategory(category.id)}
                      whileHover={{ scale: 1.02 }}
                      whileTap={{ scale: 0.98 }}
                      data-testid={`category-${category.id}`}
                    >
                      <Icon className="w-5 h-5" />
                      <span className="text-sm italic-forward">{category.name}</span>
                    </motion.button>
                  );
                })}
              </nav>
            </div>
          </motion.div>

          {/* Main Content - Settings */}
          <motion.div 
            className="lg:col-span-9"
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.3, delay: 0.1 }}
          >
            <div className="bg-gray-900/80 border border-gray-700 backdrop-blur-sm">
              <div className="p-6 border-b border-gray-700">
                <h2 className="text-xl font-bold italic-forward">
                  {settingsCategories.find(cat => cat.id === activeCategory)?.name}
                </h2>
              </div>
              
              <div className="p-6">
                <AnimatePresence mode="wait">
                  <motion.div
                    key={activeCategory}
                    initial={{ opacity: 0, x: 20 }}
                    animate={{ opacity: 1, x: 0 }}
                    exit={{ opacity: 0, x: -20 }}
                    transition={{ duration: 0.2 }}
                  >
                    {renderSettingsContent()}
                  </motion.div>
                </AnimatePresence>
              </div>
            </div>
          </motion.div>
        </div>
      </div>

      {/* Reset Confirmation Dialog */}
      <AnimatePresence>
        {showResetDialog && (
          <motion.div
            className="fixed inset-0 z-50"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
          >
            <div className="fixed inset-0 bg-black/80 backdrop-blur-sm" onClick={() => setShowResetDialog(false)} />
            
            <div className="fixed inset-0 flex items-center justify-center p-4">
              <motion.div
                className="bg-gray-900 border-2 border-electric-lime w-full max-w-md p-6"
                initial={{ opacity: 0, scale: 0.9 }}
                animate={{ opacity: 1, scale: 1 }}
                exit={{ opacity: 0, scale: 0.9 }}
              >
                <h3 className="text-lg font-bold mb-4">Reset Settings</h3>
                <p className="text-gray-300 mb-6">Choose how you want to reset your settings:</p>
                
                <div className="flex flex-col space-y-3">
                  <Button
                    onClick={handleReset}
                    className="btn-secondary justify-center"
                    data-testid="reset-to-last-saved"
                  >
                    Reset to Last Saved
                  </Button>
                  
                  <Button
                    onClick={handleResetToDefaults}
                    className="bg-red-600 hover:bg-red-700 text-white px-4 py-2 justify-center"
                    data-testid="reset-to-defaults"
                  >
                    Reset to Defaults
                  </Button>
                  
                  <Button
                    onClick={() => setShowResetDialog(false)}
                    className="bg-gray-700 hover:bg-gray-600 text-white px-4 py-2 justify-center"
                    data-testid="cancel-reset"
                  >
                    Cancel
                  </Button>
                </div>
              </motion.div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}