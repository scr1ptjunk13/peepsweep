import { Switch, Route } from "wouter";
import { queryClient } from "./lib/queryClient";
import { QueryClientProvider } from "@tanstack/react-query";
import { Toaster } from "@/components/ui/toaster";
import { TooltipProvider } from "@/components/ui/tooltip";
import { WalletProvider } from "@/lib/wallet-context";
import Home from "@/pages/home";
import Portfolio from "@/pages/portfolio";
import Analytics from "@/pages/analytics";
import Settings from "@/pages/settings";
import Connect from "@/pages/connect";
import About from "@/pages/about";
import NotFound from "@/pages/not-found";

function Router() {
  return (
    <Switch>
      <Route path="/" component={Home} />
      <Route path="/portfolio" component={Portfolio} />
      <Route path="/analytics" component={Analytics} />
      <Route path="/settings" component={Settings} />
      <Route path="/connect" component={Connect} />
      <Route path="/about" component={About} />
      <Route component={NotFound} />
    </Switch>
  );
}

function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <WalletProvider>
        <TooltipProvider>
          <Toaster />
          <div className="min-h-screen bg-black relative">
            {/* Animated Grid Background */}
            <div className="fixed inset-0 animated-grid opacity-20 pointer-events-none" />
            <Router />
          </div>
        </TooltipProvider>
      </WalletProvider>
    </QueryClientProvider>
  );
}

export default App;
