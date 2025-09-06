import { motion } from "framer-motion";
import Header from "@/components/header";
import SwapInterface from "@/components/swap-interface";
import PerformanceSidebar from "@/components/performance-sidebar";

export default function Home() {
  return (
    <div className="relative z-10">
      <Header />
      
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
        <div className="grid grid-cols-1 lg:grid-cols-12 gap-8">
          {/* Main Swap Interface */}
          <motion.div 
            className="lg:col-span-8"
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.3, ease: "easeOut" }}
          >
            <SwapInterface />
          </motion.div>
          
          {/* Performance Sidebar */}
          <motion.div 
            className="lg:col-span-4"
            initial={{ opacity: 0, x: 20 }}
            animate={{ opacity: 1, x: 0 }}
            transition={{ duration: 0.3, ease: "easeOut", delay: 0.1 }}
          >
            <PerformanceSidebar />
          </motion.div>
        </div>
      </div>
    </div>
  );
}
