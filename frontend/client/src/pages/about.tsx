import { useState, useEffect } from "react";
import { motion, useInView, AnimatePresence } from "framer-motion";
import { 
  Zap,
  Shield,
  TrendingUp,
  Users,
  Award,
  ExternalLink,
  CheckCircle,
  ArrowRight,
  Github,
  Twitter,
  MessageCircle,
  Mail,
  BarChart3,
  Clock,
  DollarSign,
  Globe,
  Code,
  Lock
} from "lucide-react";
import { Button } from "@/components/ui/button";
import Header from "@/components/header";
import { useRef } from "react";

interface Metric {
  value: string;
  label: string;
  icon: React.ComponentType<any>;
  color: string;
  trend?: string;
}

const keyMetrics: Metric[] = [
  { value: '18ms', label: 'Average Quote Speed', icon: Zap, color: 'text-electric-lime', trend: '+15% faster' },
  { value: '99.4%', label: 'Success Rate', icon: CheckCircle, color: 'text-velocity-green', trend: 'Industry leading' },
  { value: '$2.4M', label: 'Daily Volume', icon: BarChart3, color: 'text-nuclear-blue', trend: '+47% this month' },
  { value: '15K+', label: 'Active Traders', icon: Users, color: 'text-lightning-yellow', trend: 'Growing daily' }
];

const features = [
  {
    title: 'Lightning-Fast Quotes',
    description: 'Get swap quotes in under 50ms on average, 3x faster than competitors',
    icon: Zap,
    color: 'electric-lime'
  },
  {
    title: 'Smart Route Optimization',
    description: 'AI-powered routing finds the best prices across 50+ DEXs automatically',
    icon: TrendingUp,
    color: 'nuclear-blue'
  },
  {
    title: 'MEV Protection',
    description: 'Advanced protection against frontrunning and sandwich attacks',
    icon: Shield,
    color: 'velocity-green'
  },
  {
    title: 'Cross-Chain Support',
    description: 'Trade seamlessly across Ethereum, Polygon, Arbitrum, and more',
    icon: Globe,
    color: 'lightning-yellow'
  }
];

const techStack = [
  { name: 'Ethereum', description: 'Primary blockchain network' },
  { name: 'Polygon', description: 'Layer 2 scaling solution' },
  { name: 'Arbitrum', description: 'Optimistic rollup network' },
  { name: 'Optimism', description: 'Fast Ethereum L2' },
  { name: 'Uniswap V3', description: 'Concentrated liquidity DEX' },
  { name: 'SushiSwap', description: 'Community-driven DEX' },
  { name: 'Curve', description: 'Stablecoin-focused AMM' },
  { name: 'Balancer', description: 'Weighted pool protocol' }
];

const teamMembers = [
  {
    name: 'Alex Chen',
    role: 'Founder & CEO',
    bio: 'Former lead engineer at 1inch with 8 years in DeFi',
    avatar: 'bg-gradient-to-br from-electric-lime to-nuclear-blue'
  },
  {
    name: 'Sarah Kim',
    role: 'CTO',
    bio: 'Ex-Uniswap protocol developer, MEV research expert',
    avatar: 'bg-gradient-to-br from-nuclear-blue to-velocity-green'
  },
  {
    name: 'Marcus Rodriguez',
    role: 'Head of Security',
    bio: 'Security researcher with multiple DeFi audit experience',
    avatar: 'bg-gradient-to-br from-velocity-green to-lightning-yellow'
  }
];

const faqs = [
  {
    question: 'How is HyperDEX faster than competitors?',
    answer: 'We use advanced caching, parallel processing, and direct RPC connections to minimize latency. Our proprietary routing algorithm pre-computes optimal paths.'
  },
  {
    question: 'What fees does HyperDEX charge?',
    answer: 'HyperDEX charges a 0.1% fee on successful swaps. This is lower than most competitors and includes MEV protection at no extra cost.'
  },
  {
    question: 'Is my wallet connection secure?',
    answer: 'Yes, HyperDEX never stores your private keys. We use industry-standard wallet connection protocols and have undergone multiple security audits.'
  },
  {
    question: 'Which networks are supported?',
    answer: 'Currently Ethereum, Polygon, Arbitrum, and Optimism. We\'re adding more networks based on user demand and liquidity availability.'
  },
  {
    question: 'How does MEV protection work?',
    answer: 'Our MEV protection routes transactions through private mempools and uses commit-reveal schemes to prevent frontrunning and sandwich attacks.'
  }
];

export default function About() {
  const [activeMetric, setActiveMetric] = useState(0);
  const [activeFaq, setActiveFaq] = useState<number | null>(null);
  const heroRef = useRef(null);
  const metricsRef = useRef(null);
  const featuresRef = useRef(null);
  
  const heroInView = useInView(heroRef, { once: true });
  const metricsInView = useInView(metricsRef, { once: true });
  const featuresInView = useInView(featuresRef, { once: true });

  // Rotate active metric every 3 seconds
  useEffect(() => {
    const interval = setInterval(() => {
      setActiveMetric((prev) => (prev + 1) % keyMetrics.length);
    }, 3000);
    return () => clearInterval(interval);
  }, []);

  const CountingNumber = ({ value, duration = 2000 }: { value: string; duration?: number }) => {
    const [displayValue, setDisplayValue] = useState('0');
    const ref = useRef(null);
    const inView = useInView(ref, { once: true });

    useEffect(() => {
      if (!inView) return;

      const numericValue = parseFloat(value.replace(/[^0-9.]/g, ''));
      const suffix = value.replace(/[0-9.]/g, '');
      
      if (isNaN(numericValue)) {
        setDisplayValue(value);
        return;
      }

      let start = 0;
      const increment = numericValue / (duration / 50);
      
      const timer = setInterval(() => {
        start += increment;
        if (start >= numericValue) {
          setDisplayValue(value);
          clearInterval(timer);
        } else {
          setDisplayValue(Math.floor(start) + suffix);
        }
      }, 50);

      return () => clearInterval(timer);
    }, [inView, value, duration]);

    return <span ref={ref}>{displayValue}</span>;
  };

  return (
    <div className="relative z-10">
      <Header />
      
      {/* Hero Section */}
      <section ref={heroRef} className="relative min-h-[80vh] flex items-center">
        <div className="absolute inset-0 bg-gradient-to-b from-transparent via-black/20 to-black" />
        
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 relative z-10">
          <motion.div
            className="text-center"
            initial={{ opacity: 0, y: 50 }}
            animate={heroInView ? { opacity: 1, y: 0 } : {}}
            transition={{ duration: 0.8 }}
          >
            <h1 className="text-6xl md:text-8xl font-bold italic-forward mb-6 bg-gradient-to-r from-electric-lime via-nuclear-blue to-lightning-yellow bg-clip-text text-transparent">
              The Fastest DEX
            </h1>
            <h2 className="text-4xl md:text-6xl font-bold italic-forward mb-8">
              Aggregator Ever Built
            </h2>
            
            <p className="text-xl md:text-2xl text-gray-300 mb-8 max-w-3xl mx-auto leading-relaxed">
              Experience lightning-fast token swaps with 18ms average quotes and 99.4% success rate.
              Built for traders who demand the ultimate in speed and reliability.
            </p>
            
            <div className="flex flex-col sm:flex-row items-center justify-center space-y-4 sm:space-y-0 sm:space-x-6 mb-12">
              <Button className="btn-lightning text-lg px-8 py-4" data-testid="start-trading-hero">
                <Zap className="w-5 h-5 mr-2" />
                Start Trading Now
              </Button>
              <Button className="btn-secondary text-lg px-8 py-4" data-testid="view-analytics">
                <BarChart3 className="w-5 h-5 mr-2" />
                View Analytics
              </Button>
            </div>
            
            <div className="flex items-center justify-center space-x-8 text-sm text-gray-400">
              <div className="flex items-center space-x-2">
                <Clock className="w-4 h-4 text-electric-lime" />
                <span>Sub-50ms quotes</span>
              </div>
              <div className="flex items-center space-x-2">
                <Shield className="w-4 h-4 text-velocity-green" />
                <span>MEV protected</span>
              </div>
              <div className="flex items-center space-x-2">
                <Award className="w-4 h-4 text-nuclear-blue" />
                <span>Audited & secure</span>
              </div>
            </div>
          </motion.div>
        </div>
      </section>

      {/* Key Metrics */}
      <section ref={metricsRef} className="py-20 bg-gray-900/50">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
          <motion.div
            className="text-center mb-16"
            initial={{ opacity: 0, y: 30 }}
            animate={metricsInView ? { opacity: 1, y: 0 } : {}}
            transition={{ duration: 0.6 }}
          >
            <h2 className="text-4xl font-bold italic-forward mb-4">Performance That Speaks</h2>
            <p className="text-xl text-gray-400">Real metrics from real traders</p>
          </motion.div>
          
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-8">
            {keyMetrics.map((metric, index) => {
              const Icon = metric.icon;
              return (
                <motion.div
                  key={index}
                  className={`bg-gray-900/80 border backdrop-blur-sm p-8 text-center transition-all duration-300 ${
                    activeMetric === index 
                      ? 'border-electric-lime/50 electric-glow' 
                      : 'border-gray-700 hover:border-gray-600'
                  }`}
                  initial={{ opacity: 0, y: 30 }}
                  animate={metricsInView ? { opacity: 1, y: 0 } : {}}
                  transition={{ duration: 0.6, delay: index * 0.1 }}
                >
                  <Icon className={`w-8 h-8 ${metric.color} mx-auto mb-4`} />
                  <div className={`text-3xl font-bold font-mono mb-2 ${metric.color}`}>
                    <CountingNumber value={metric.value} />
                  </div>
                  <div className="text-sm text-gray-300 mb-2">{metric.label}</div>
                  {metric.trend && (
                    <div className="text-xs text-velocity-green font-medium">
                      {metric.trend}
                    </div>
                  )}
                </motion.div>
              );
            })}
          </div>
        </div>
      </section>

      {/* Features Overview */}
      <section ref={featuresRef} className="py-20">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
          <motion.div
            className="text-center mb-16"
            initial={{ opacity: 0, y: 30 }}
            animate={featuresInView ? { opacity: 1, y: 0 } : {}}
            transition={{ duration: 0.6 }}
          >
            <h2 className="text-4xl font-bold italic-forward mb-4">Built for Speed</h2>
            <p className="text-xl text-gray-400">Every millisecond matters in DeFi</p>
          </motion.div>
          
          <div className="grid grid-cols-1 md:grid-cols-2 gap-8 mb-16">
            {features.map((feature, index) => {
              const Icon = feature.icon;
              return (
                <motion.div
                  key={index}
                  className="bg-gray-900/80 border border-gray-700 hover:border-gray-600 backdrop-blur-sm p-8 transition-all duration-300"
                  initial={{ opacity: 0, x: index % 2 === 0 ? -30 : 30 }}
                  animate={featuresInView ? { opacity: 1, x: 0 } : {}}
                  transition={{ duration: 0.6, delay: index * 0.2 }}
                >
                  <Icon className={`w-8 h-8 text-${feature.color} mb-4`} />
                  <h3 className="text-xl font-bold mb-3">{feature.title}</h3>
                  <p className="text-gray-400 leading-relaxed">{feature.description}</p>
                </motion.div>
              );
            })}
          </div>
        </div>
      </section>

      {/* Technology Stack */}
      <section className="py-20 bg-gray-900/50">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
          <motion.div
            className="text-center mb-16"
            initial={{ opacity: 0, y: 30 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.6 }}
            viewport={{ once: true }}
          >
            <h2 className="text-4xl font-bold italic-forward mb-4">Technology Stack</h2>
            <p className="text-xl text-gray-400">Built on the most advanced DeFi infrastructure</p>
          </motion.div>
          
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
            {techStack.map((tech, index) => (
              <motion.div
                key={index}
                className="bg-black/40 border border-gray-700 hover:border-electric-lime/30 p-4 text-center transition-all duration-200"
                initial={{ opacity: 0, scale: 0.9 }}
                animate={{ opacity: 1, scale: 1 }}
                transition={{ duration: 0.4, delay: index * 0.05 }}
                viewport={{ once: true }}
                whileHover={{ scale: 1.05 }}
              >
                <div className="font-bold text-sm mb-1">{tech.name}</div>
                <div className="text-xs text-gray-400">{tech.description}</div>
              </motion.div>
            ))}
          </div>
        </div>
      </section>

      {/* Team Section */}
      <section className="py-20">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
          <motion.div
            className="text-center mb-16"
            initial={{ opacity: 0, y: 30 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.6 }}
            viewport={{ once: true }}
          >
            <h2 className="text-4xl font-bold italic-forward mb-4">The Team</h2>
            <p className="text-xl text-gray-400">DeFi veterans building the future of trading</p>
          </motion.div>
          
          <div className="grid grid-cols-1 md:grid-cols-3 gap-8">
            {teamMembers.map((member, index) => (
              <motion.div
                key={index}
                className="text-center"
                initial={{ opacity: 0, y: 30 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.6, delay: index * 0.1 }}
                viewport={{ once: true }}
              >
                <div className={`w-24 h-24 ${member.avatar} rounded-full mx-auto mb-4`} />
                <h3 className="text-xl font-bold mb-1">{member.name}</h3>
                <div className="text-electric-lime mb-2">{member.role}</div>
                <p className="text-gray-400 text-sm">{member.bio}</p>
              </motion.div>
            ))}
          </div>
        </div>
      </section>

      {/* FAQ Section */}
      <section className="py-20 bg-gray-900/50">
        <div className="max-w-4xl mx-auto px-4 sm:px-6 lg:px-8">
          <motion.div
            className="text-center mb-16"
            initial={{ opacity: 0, y: 30 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.6 }}
            viewport={{ once: true }}
          >
            <h2 className="text-4xl font-bold italic-forward mb-4">Frequently Asked Questions</h2>
            <p className="text-xl text-gray-400">Everything you need to know about HyperDEX</p>
          </motion.div>
          
          <div className="space-y-4">
            {faqs.map((faq, index) => (
              <motion.div
                key={index}
                className="bg-gray-900/80 border border-gray-700 backdrop-blur-sm"
                initial={{ opacity: 0, y: 20 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.4, delay: index * 0.1 }}
                viewport={{ once: true }}
              >
                <button
                  className="w-full p-6 text-left flex items-center justify-between hover:bg-gray-800/50 transition-colors duration-200"
                  onClick={() => setActiveFaq(activeFaq === index ? null : index)}
                  data-testid={`faq-${index}`}
                >
                  <span className="font-bold">{faq.question}</span>
                  <motion.div
                    animate={{ rotate: activeFaq === index ? 45 : 0 }}
                    transition={{ duration: 0.2 }}
                  >
                    <ArrowRight className="w-5 h-5 text-electric-lime" />
                  </motion.div>
                </button>
                
                <AnimatePresence>
                  {activeFaq === index && (
                    <motion.div
                      initial={{ height: 0, opacity: 0 }}
                      animate={{ height: 'auto', opacity: 1 }}
                      exit={{ height: 0, opacity: 0 }}
                      transition={{ duration: 0.3 }}
                      className="overflow-hidden border-t border-gray-700"
                    >
                      <div className="p-6 text-gray-400">{faq.answer}</div>
                    </motion.div>
                  )}
                </AnimatePresence>
              </motion.div>
            ))}
          </div>
        </div>
      </section>

      {/* Contact Section */}
      <section className="py-20">
        <div className="max-w-4xl mx-auto px-4 sm:px-6 lg:px-8 text-center">
          <motion.div
            initial={{ opacity: 0, y: 30 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.6 }}
            viewport={{ once: true }}
          >
            <h2 className="text-4xl font-bold italic-forward mb-4">Get In Touch</h2>
            <p className="text-xl text-gray-400 mb-8">
              Questions? Feedback? We'd love to hear from you.
            </p>
            
            <div className="grid grid-cols-1 md:grid-cols-4 gap-6 mb-12">
              <a
                href="mailto:hello@hyperdex.com"
                className="flex flex-col items-center p-6 bg-gray-900/80 border border-gray-700 hover:border-electric-lime/30 backdrop-blur-sm transition-all duration-200 group"
                data-testid="contact-email"
              >
                <Mail className="w-8 h-8 text-electric-lime mb-3 group-hover:scale-110 transition-transform" />
                <span className="text-sm font-bold">Email</span>
                <span className="text-xs text-gray-400">hello@hyperdex.com</span>
              </a>
              
              <a
                href="#"
                className="flex flex-col items-center p-6 bg-gray-900/80 border border-gray-700 hover:border-nuclear-blue/30 backdrop-blur-sm transition-all duration-200 group"
                data-testid="contact-discord"
              >
                <MessageCircle className="w-8 h-8 text-nuclear-blue mb-3 group-hover:scale-110 transition-transform" />
                <span className="text-sm font-bold">Discord</span>
                <span className="text-xs text-gray-400">Join community</span>
              </a>
              
              <a
                href="#"
                className="flex flex-col items-center p-6 bg-gray-900/80 border border-gray-700 hover:border-lightning-yellow/30 backdrop-blur-sm transition-all duration-200 group"
                data-testid="contact-twitter"
              >
                <Twitter className="w-8 h-8 text-lightning-yellow mb-3 group-hover:scale-110 transition-transform" />
                <span className="text-sm font-bold">Twitter</span>
                <span className="text-xs text-gray-400">@HyperDEX</span>
              </a>
              
              <a
                href="#"
                className="flex flex-col items-center p-6 bg-gray-900/80 border border-gray-700 hover:border-velocity-green/30 backdrop-blur-sm transition-all duration-200 group"
                data-testid="contact-github"
              >
                <Github className="w-8 h-8 text-velocity-green mb-3 group-hover:scale-110 transition-transform" />
                <span className="text-sm font-bold">GitHub</span>
                <span className="text-xs text-gray-400">Open source</span>
              </a>
            </div>
            
            <div className="flex justify-center space-x-6 text-sm text-gray-500">
              <a href="#" className="hover:text-electric-lime transition-colors duration-200">
                Privacy Policy
              </a>
              <a href="#" className="hover:text-electric-lime transition-colors duration-200">
                Terms of Service
              </a>
              <a href="#" className="hover:text-electric-lime transition-colors duration-200">
                Security Audit
              </a>
            </div>
          </motion.div>
        </div>
      </section>
    </div>
  );
}