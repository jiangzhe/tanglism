export const atr = {
    data,
    draw_table
};

const _data = [];
const table_header = {
  names: ["code", "display_name", "msci", "hs300", "atrp_days", "atrp_max", "atrp_min", "atrp_avg"],
  display_names: ["股票代码", "股票名称", "MSCI", "沪深300", "ATRP天数", "ATRP最大值", "ATRP最小值", "ATRP平均值"],
  sort_ascending: [false, false, false, false, false, false, false, false]
};


function data(input) {
    if (input) {
      while(_data.length > 0) { _data.pop(); }
      for (var i = 0; i < input.length; i++) {
        _data.push(input[i]);
      }
      return;
    }
    return _data;
};

function draw_table() {
    // 刷新表格
    if (!d3.select("#atr_table").empty()) {
      d3.select("#atr_table").remove();
    }
    var table = d3.select("#atr_container").append("table")
      .attr("id", "atr_table")
      .style("border-collapse", "collapse")
      .style("border", "2px black solid");
    // 表头
    table.append("thead")
      .append("tr")
      .selectAll("th")
      .data(table_header.display_names)
      .enter()
      .append("th")
      .text(function(d) {return d;})
      .style("border", "1px black solid")
      .style("padding", "5px")
      .style("background-color", "lightgray")
      .style("font-weight", "bold")
      .on("click", function(d, i){
          // 重排序
          const name = table_header.names[i];
          var cmp;
          if (name === "atrp_max" || name === "atrp_min" || name === "atrp_avg") {
            cmp = function(a, b) {
              if (a[name] == undefined) {
                return -1;
              }
              if (b[name] == undefined) {
                return 1;
              }
              var av = parseFloat(a[name]);
              var bv = parseFloat(b[name]);
              if (av < bv) {
                return -1;
              }
              if (av > bv) {
                return 1;
              }
              return 0;
            };
          } else {
            cmp = function(a,b) {
              if (a[name] === undefined) {
                return -1;
              }
              if (b[name] === undefined) {
                return 1;
              }
              if (a[name] < b[name]) {
                return -1;
              }
              if (a[name] > b[name]) {
                return 1;
              }
              return 0;
            }
          }
          // 正序和逆序交错
          if (!table_header.sort_ascending[i]) {
            var tmp = cmp;
            cmp = function(a, b) {
              return tmp(b, a);
            }
          }
          _data.sort(cmp);
          draw_table();
          table_header.sort_ascending[i] = !table_header.sort_ascending[i];
      });
    
    // 内容
    table.append("tbody")
      .selectAll("tr")
      .data(_data)
      .enter()
      .append("tr")
      .selectAll("td")
      .data(function(d) {
        var msci = "N";
        if (d.msci) { msci = "Y"; }
        var hs300 = "N";
        if (d.hs300) { hs300 = "Y"; }
        var atrp_max = "";
        if (d.atrp_max) { atrp_max = (parseFloat(d.atrp_max) * 100).toFixed(2); }
        var atrp_min = "";
        if (d.atrp_min) { atrp_min = (parseFloat(d.atrp_min) * 100).toFixed(2); }
        var atrp_avg = "";
        if (d.atrp_avg) { atrp_avg = (parseFloat(d.atrp_avg) * 100).toFixed(2); }
        return [
          d.code, 
          d.display_name, 
          msci,
          hs300,
          d.atrp_days, 
          atrp_max,
          atrp_min,
          atrp_avg
        ];
      })
      .enter()
      .append("td")
      .style("border", "1px black solid")
      .style("padding", "5px")
      .style("font-size", "12px")
      .text(function(d) {return d;})
      .on("mouseover", function(){
        d3.select(this).style("background-color", "powderblue");
      })
      .on("mouseout", function(){
        d3.select(this).style("background-color", "white");
      });
};
