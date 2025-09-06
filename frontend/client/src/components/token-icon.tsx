import { useState } from "react";
import { priceService } from "@/lib/price-service";
import { chainLogoService } from "@/lib/chain-logo-service";

interface TokenIconProps {
  symbol: string;
  size?: number;
  className?: string;
  fallbackGradient?: string;
  chainId?: number;
  showChainBadge?: boolean;
}

export default function TokenIcon({ 
  symbol, 
  size = 24, 
  className = "", 
  fallbackGradient = "bg-gradient-to-br from-gray-500 to-gray-700",
  chainId,
  showChainBadge = false
}: TokenIconProps) {
  const [imageError, setImageError] = useState(false);
  const [imageLoaded, setImageLoaded] = useState(false);
  
  const imageUrl = priceService.getTokenImage(symbol);
  
  // Chain badge configuration with real logos
  const chainInfo = chainId ? chainLogoService.getChainInfo(chainId) : null;
  const chainLogo = chainId ? chainLogoService.getChainLogo(chainId) : null;
  
  const handleImageError = () => {
    setImageError(true);
  };
  
  const handleImageLoad = () => {
    setImageLoaded(true);
  };

  // If we have an image URL and no error, show the image
  if (imageUrl && !imageError) {
    return (
      <div className={`relative ${className}`} style={{ width: size, height: size }}>
        <img
          src={imageUrl}
          alt={`${symbol} logo`}
          className={`w-full h-full rounded-full object-cover transition-opacity duration-200 ${
            imageLoaded ? 'opacity-100' : 'opacity-0'
          }`}
          onError={handleImageError}
          onLoad={handleImageLoad}
        />
        {!imageLoaded && (
          <div 
            className={`absolute inset-0 rounded-full ${fallbackGradient} animate-pulse`}
          />
        )}
        
        {/* Chain Badge - Bottom Left Corner (like 1inch) */}
        {showChainBadge && chainInfo && (
          <div 
            className="absolute -bottom-1 -left-1 w-4 h-4 rounded-full border-2 border-gray-900 shadow-lg overflow-hidden"
            title={chainInfo.name}
          >
            {chainLogo ? (
              <img 
                src={chainLogo}
                alt={chainInfo.name}
                className="w-full h-full object-cover"
                onError={(e) => {
                  // Fallback to colored background with first letter
                  const target = e.target as HTMLImageElement;
                  target.style.display = 'none';
                  const fallback = target.nextElementSibling as HTMLElement;
                  if (fallback) fallback.style.display = 'flex';
                }}
              />
            ) : null}
            <div 
              className="w-full h-full flex items-center justify-center text-[8px] font-bold text-white"
              style={{ 
                backgroundColor: chainInfo.color,
                display: chainLogo ? 'none' : 'flex'
              }}
            >
              {chainInfo.symbol.charAt(0)}
            </div>
          </div>
        )}
      </div>
    );
  }

  // Fallback to gradient background
  return (
    <div className={`relative ${className}`} style={{ width: size, height: size }}>
      <div 
        className={`w-full h-full rounded-full flex items-center justify-center text-white font-bold text-xs ${fallbackGradient}`}
      >
        {symbol.slice(0, 2)}
      </div>
      
      {/* Chain Badge - Bottom Left Corner (like 1inch) */}
      {showChainBadge && chainInfo && (
        <div 
          className="absolute -bottom-1 -left-1 w-4 h-4 rounded-full border-2 border-gray-900 shadow-lg overflow-hidden"
          title={chainInfo.name}
        >
          {chainLogo ? (
            <img 
              src={chainLogo}
              alt={chainInfo.name}
              className="w-full h-full object-cover"
              onError={(e) => {
                // Fallback to colored background with first letter
                const target = e.target as HTMLImageElement;
                target.style.display = 'none';
                const fallback = target.nextElementSibling as HTMLElement;
                if (fallback) fallback.style.display = 'flex';
              }}
            />
          ) : null}
          <div 
            className="w-full h-full flex items-center justify-center text-[8px] font-bold text-white"
            style={{ 
              backgroundColor: chainInfo.color,
              display: chainLogo ? 'none' : 'flex'
            }}
          >
            {chainInfo.symbol.charAt(0)}
          </div>
        </div>
      )}
    </div>
  );
}
